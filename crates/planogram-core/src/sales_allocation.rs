//! Bounded sales allocation for existing placements. This is a deterministic
//! weighted incremental heuristic, not a mathematical global optimum or a sales
//! forecast. Scope, assortment, shelf assignment and tray presets stay fixed.

use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SalesAllocationScope {
    Shelf { shelf_id: ShelfId },
    Bay { section_id: SectionId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesAllocationBasis {
    Revenue,
    Units,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesAllocationTarget {
    Space,
    Facings,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesAllocationRequest {
    pub scope: SalesAllocationScope,
    pub basis: SalesAllocationBasis,
    pub target: SalesAllocationTarget,
    /// Bounds apply to each existing loose placement, not to a SKU's total.
    pub min_facings: u32,
    pub max_facings: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SalesAllocationRow {
    pub product_id: ProductId,
    pub contribution_basis_points: u32,
    pub before_facings: u32,
    pub after_facings: u32,
    pub before_space_sixteenths: i64,
    pub after_space_sixteenths: i64,
    pub before_share_basis_points: u32,
    pub after_share_basis_points: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SalesAllocationReport {
    pub basis: SalesAllocationBasis,
    pub target: SalesAllocationTarget,
    pub scope: SalesAllocationScope,
    /// Distinct represented SKUs, including fixed trays; demand is counted once.
    pub product_count: usize,
    /// Occupied adjustable shelves in scope.
    pub shelf_count: usize,
    pub fixed_tray_count: usize,
    pub zero_weight_sku_count: usize,
    pub period: String,
    pub source: String,
    pub rows: Vec<SalesAllocationRow>,
    pub warnings: Vec<String>,
}

struct PlannedSalesPlacement<'a> {
    before: &'a Placement,
    product_index: usize,
    facings_x: u32,
}

struct PlannedSalesShelf<'a> {
    shelf: &'a Shelf,
    placements: Vec<PlannedSalesPlacement<'a>>,
    widths: Vec<Length>,
    joined: Vec<bool>,
}

impl DraftVersion {
    pub fn preview_sales_allocation(
        &self,
        version_id: &VersionId,
        request: &SalesAllocationRequest,
        expected_revision: u64,
    ) -> PreviewResult {
        if let Err(result) = self.check_sales_allocation_revision(version_id, expected_revision) {
            return sales_preview_error(result);
        }
        let (changes, report) = match self.resolve_sales_allocation(request) {
            Ok(resolved) => resolved,
            Err(result) => return sales_preview_error(result),
        };
        match self.prepare_placement_changes(&changes, ShelfLayout::Recorded) {
            Ok(prepared) => PreviewResult::Ready {
                revision: self.revision,
                operations: prepared.operations,
                affected_ids: prepared.affected_ids,
                validation: ValidationSummary::default(),
                preview_scene: Box::new(prepared.candidate.render_scene()),
                sales_allocation: Some(Box::new(report)),
            },
            Err(result) => sales_preview_error(result),
        }
    }

    pub fn apply_sales_allocation_as(
        &mut self,
        version_id: &VersionId,
        request: &SalesAllocationRequest,
        expected_revision: u64,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> CommandResult {
        if let Err(result) = self.check_sales_allocation_revision(version_id, expected_revision) {
            return result;
        }
        // Recompute semantic intent at the guarded revision. A client's preview
        // is never trusted as a set of final coordinates or facing counts.
        let (changes, _) = match self.resolve_sales_allocation(request) {
            Ok(resolved) => resolved,
            Err(result) => return result,
        };
        self.apply_placement_changes_with_compensation(
            version_id,
            &changes,
            expected_revision,
            ChangeSetMetadata {
                actor: actor.into(),
                reason: reason.into(),
                compensates: None,
            },
            ShelfLayout::Recorded,
        )
    }

    #[allow(clippy::result_large_err)]
    fn check_sales_allocation_revision(
        &self,
        version_id: &VersionId,
        expected_revision: u64,
    ) -> Result<(), CommandResult> {
        if version_id != &self.id {
            return Err(CommandResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            });
        }
        if expected_revision != self.revision {
            return Err(CommandResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            });
        }
        if !self.status.is_editable() {
            return Err(CommandResult::Forbidden {
                message: "This version is not editable.".into(),
            });
        }
        Ok(())
    }

    fn sales_allocation_invalid(
        &self,
        code: ValidationCode,
        shelf_id: Option<ShelfId>,
        message: impl Into<String>,
    ) -> CommandResult {
        CommandResult::ValidationFailed {
            revision: self.revision,
            validation: ValidationSummary {
                issues: vec![ValidationIssue {
                    code,
                    shelf_id,
                    message: message.into(),
                }],
            },
        }
    }

    #[allow(clippy::result_large_err)]
    fn sales_allocation_shelves(
        &self,
        scope: &SalesAllocationScope,
    ) -> Result<Vec<&Shelf>, CommandResult> {
        let mut shelves = match scope {
            SalesAllocationScope::Shelf { shelf_id } => {
                let shelf = self
                    .shelf(shelf_id)
                    .ok_or_else(|| CommandResult::NotFound {
                        entity: "shelf".into(),
                        id: shelf_id.0.clone(),
                    })?;
                if shelf.kind != ShelfKind::Adjustable {
                    return Err(self.sales_allocation_invalid(
                        ValidationCode::PlacementOnFixedShelf,
                        Some(shelf.id.clone()),
                        "The fixed base deck cannot allocate product facings.",
                    ));
                }
                vec![shelf]
            }
            SalesAllocationScope::Bay { section_id } => self
                .fixture
                .sections
                .iter()
                .find(|section| section.id == *section_id)
                .ok_or_else(|| CommandResult::NotFound {
                    entity: "section".into(),
                    id: section_id.0.clone(),
                })?
                .shelves
                .iter()
                .filter(|shelf| shelf.kind == ShelfKind::Adjustable)
                .collect(),
        };
        shelves.retain(|shelf| {
            self.placements
                .iter()
                .any(|placement| placement.shelf_id == shelf.id)
        });
        shelves.sort_by(|left, right| {
            left.elevation
                .cmp(&right.elevation)
                .then_with(|| left.id.cmp(&right.id))
        });
        if shelves.is_empty() {
            return Err(CommandResult::InvalidCommand {
                message: "The selected scope has no products on adjustable shelves to allocate."
                    .into(),
            });
        }
        Ok(shelves)
    }

    #[allow(clippy::result_large_err)]
    fn resolve_sales_allocation(
        &self,
        request: &SalesAllocationRequest,
    ) -> Result<(Vec<PlacementChange>, SalesAllocationReport), CommandResult> {
        if !(1..=100).contains(&request.min_facings)
            || !(1..=100).contains(&request.max_facings)
            || request.min_facings > request.max_facings
        {
            return Err(self.sales_allocation_invalid(
                ValidationCode::InvalidFacingCount,
                None,
                "Minimum and maximum horizontal facings must be between 1 and 100, with minimum no greater than maximum.",
            ));
        }
        let shelves = self.sales_allocation_shelves(&request.scope)?;
        let mut represented = BTreeMap::new();
        for placement in &self.placements {
            if !shelves.iter().any(|shelf| shelf.id == placement.shelf_id) {
                continue;
            }
            let product =
                self.product(&placement.product_id)
                    .ok_or_else(|| CommandResult::NotFound {
                        entity: "product".into(),
                        id: placement.product_id.0.clone(),
                    })?;
            represented.insert(product.id.clone(), product);
        }
        // Product-ID ordering also gives report rounding a stable tie-breaker.
        let products = represented.into_values().collect::<Vec<_>>();
        let mut validation = ValidationSummary::default();
        for product in &products {
            validation
                .issues
                .extend(Self::validate_product(product).issues);
            if self
                .products
                .iter()
                .filter(|item| item.id == product.id)
                .count()
                != 1
            {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::DuplicateProductId,
                    shelf_id: None,
                    message: format!("Product ID {} appears more than once.", product.id.0),
                });
            }
        }
        if !validation.valid() {
            return Err(CommandResult::ValidationFailed {
                revision: self.revision,
                validation,
            });
        }
        let period = products[0].performance.period.trim();
        // Generator 1 stores a per-SKU illustrative price after its common
        // source. Recognize only that exact, versioned provenance format; the
        // immutable catalog and its Save/Open history must remain unchanged.
        let cereal_source = self
            .scenario_origin
            .as_ref()
            .filter(|origin| origin.generator_version == 1)
            .map(|_| cereal_assumptions().source);
        let source = sales_source_family(&products[0].performance.source, cereal_source.as_deref());
        if products.iter().any(|product| {
            product.performance.period.trim() != period
                || sales_source_family(&product.performance.source, cereal_source.as_deref())
                    != source
        }) {
            return Err(self.sales_allocation_invalid(
                ValidationCode::InvalidProductPerformance,
                None,
                "All represented products must use the same nonblank performance period and source before their contributions can be compared.",
            ));
        }
        let weights = products
            .iter()
            .map(|product| match request.basis {
                SalesAllocationBasis::Revenue => product.performance.sales_per_store_per_week_cents,
                SalesAllocationBasis::Units => {
                    product.performance.units_per_store_per_week_milliunits
                }
            })
            .collect::<Vec<_>>();
        if weights.iter().all(|weight| *weight == 0) {
            return Err(self.sales_allocation_invalid(
                ValidationCode::InvalidProductPerformance,
                None,
                "The selected contribution basis is zero for every represented product; allocation needs at least one positive value.",
            ));
        }

        let mut planned = Vec::with_capacity(shelves.len());
        let mut measures = vec![0_u128; products.len()];
        let mut rows = products
            .iter()
            .map(|product| SalesAllocationRow {
                product_id: product.id.clone(),
                contribution_basis_points: 0,
                before_facings: 0,
                after_facings: 0,
                before_space_sixteenths: 0,
                after_space_sixteenths: 0,
                before_share_basis_points: 0,
                after_share_basis_points: 0,
            })
            .collect::<Vec<_>>();
        let mut fixed_tray_count = 0;
        let mut loose_count = 0;
        for shelf in shelves {
            let mut ordered = self
                .placements
                .iter()
                .filter(|placement| placement.shelf_id == shelf.id)
                .collect::<Vec<_>>();
            ordered
                .sort_by(|left, right| left.x.cmp(&right.x).then_with(|| left.id.cmp(&right.id)));
            let joined = same_sku_runs(ordered.iter().map(|placement| &placement.product_id));
            let mut placements = Vec::with_capacity(ordered.len());
            let mut widths = Vec::with_capacity(ordered.len());
            for before in ordered {
                let product_index = products
                    .binary_search_by(|product| product.id.cmp(&before.product_id))
                    .expect("represented products came from these placements");
                let product = products[product_index];
                if !before.facings().in_range() {
                    return Err(self.invalid_facing_count(&shelf.id));
                }
                let before_width = self.checked_sales_width(before, product)?;
                let mut after = before.clone();
                if product.tray.is_some() {
                    fixed_tray_count += 1;
                } else {
                    after.facings_x = request.min_facings;
                    loose_count += 1;
                }
                let width = self.checked_sales_width(&after, product)?;
                measures[product_index] += match request.target {
                    SalesAllocationTarget::Space => width.sixteenths() as u128,
                    SalesAllocationTarget::Facings => u128::from(after.facings_x),
                };
                if measures[product_index] > i64::MAX as u128 {
                    return Err(sales_totals_overflow());
                }
                rows[product_index].before_facings = rows[product_index]
                    .before_facings
                    .checked_add(before.facings_x)
                    .ok_or_else(sales_totals_overflow)?;
                rows[product_index].before_space_sixteenths = rows[product_index]
                    .before_space_sixteenths
                    .checked_add(i64::from(before_width.sixteenths()))
                    .ok_or_else(sales_totals_overflow)?;
                widths.push(width);
                placements.push(PlannedSalesPlacement {
                    before,
                    product_index,
                    facings_x: after.facings_x,
                });
            }
            if sales_distribution(&widths, &joined, shelf.width, ShelfDistribution::PackedLeft)
                .is_none()
            {
                return Err(self.sales_allocation_invalid(
                    ValidationCode::NoShelfCapacity,
                    Some(shelf.id.clone()),
                    "The shelf cannot fit the minimum facings of every existing loose placement plus its fixed trays, gaps and 1/8-inch position grid.",
                ));
            }
            planned.push(PlannedSalesShelf {
                shelf,
                placements,
                widths,
                joined,
            });
        }
        if loose_count == 0 {
            return Err(CommandResult::InvalidCommand {
                message: "The selected scope contains only fixed trays; there are no loose facings to allocate.".into(),
            });
        }

        // Fill one fitting facing at a time, preferring the smallest prospective
        // *total SKU* measure / demand. Integer cross-products avoid float drift.
        // Iteration order fixes ties by shelf elevation/ID, then placement x/ID.
        // Local shelf constraints and per-placement bounds can prevent exact
        // proportional shares; no SKU is removed or moved to another shelf.
        loop {
            let mut next: Option<(usize, usize, Length, u128, u64)> = None;
            for (shelf_index, shelf) in planned.iter().enumerate() {
                for (placement_index, placement) in shelf.placements.iter().enumerate() {
                    let product = products[placement.product_index];
                    let weight = weights[placement.product_index];
                    if product.tray.is_some()
                        || weight == 0
                        || placement.facings_x >= request.max_facings
                    {
                        continue;
                    }
                    let Some(next_width) = shelf.widths[placement_index]
                        .sixteenths()
                        .checked_add(product.dimensions.width.sixteenths())
                        .map(Length::from_sixteenths)
                    else {
                        continue;
                    };
                    let mut widths = shelf.widths.clone();
                    widths[placement_index] = next_width;
                    if sales_distribution(
                        &widths,
                        &shelf.joined,
                        shelf.shelf.width,
                        ShelfDistribution::PackedLeft,
                    )
                    .is_none()
                    {
                        continue;
                    }
                    let increment = match request.target {
                        SalesAllocationTarget::Space => {
                            product.dimensions.width.sixteenths() as u128
                        }
                        SalesAllocationTarget::Facings => 1,
                    };
                    let prospective = measures[placement.product_index] + increment;
                    if prospective > i64::MAX as u128 {
                        return Err(sales_totals_overflow());
                    }
                    if next.as_ref().is_none_or(|(_, _, _, best, best_weight)| {
                        prospective * u128::from(*best_weight) < *best * u128::from(weight)
                    }) {
                        next = Some((
                            shelf_index,
                            placement_index,
                            next_width,
                            prospective,
                            weight,
                        ));
                    }
                }
            }
            let Some((shelf_index, placement_index, width, measure, _)) = next else {
                break;
            };
            let shelf = &mut planned[shelf_index];
            let placement = &mut shelf.placements[placement_index];
            placement.facings_x += 1;
            shelf.widths[placement_index] = width;
            measures[placement.product_index] = measure;
        }

        let mut candidate = self.clone();
        let mut changes = Vec::new();
        for shelf in &planned {
            let positions = sales_distribution(
                &shelf.widths,
                &shelf.joined,
                shelf.shelf.width,
                ShelfDistribution::SpaceEvenly,
            )
            .ok_or_else(|| {
                self.sales_allocation_invalid(
                    ValidationCode::NoShelfCapacity,
                    Some(shelf.shelf.id.clone()),
                    "The requested sales allocation cannot fit on the shelf's position grid.",
                )
            })?;
            for ((placement, width), x) in shelf.placements.iter().zip(&shelf.widths).zip(positions)
            {
                let row = &mut rows[placement.product_index];
                row.after_facings = row
                    .after_facings
                    .checked_add(placement.facings_x)
                    .ok_or_else(sales_totals_overflow)?;
                row.after_space_sixteenths = row
                    .after_space_sixteenths
                    .checked_add(i64::from(width.sixteenths()))
                    .ok_or_else(sales_totals_overflow)?;
                let after = candidate
                    .placement_mut(&placement.before.id)
                    .expect("existing placement is preserved");
                after.facings_x = placement.facings_x;
                after.x = x;
                if after != placement.before {
                    changes.push(PlacementChange::Reflow {
                        placement_id: after.id.clone(),
                        shelf_id: after.shelf_id.clone(),
                        resolved_x: x,
                        facings_x: after.facings_x,
                        facings_y: after.facings_y,
                        facings_z: after.facings_z,
                    });
                }
            }
        }
        // Validate the complete scope, including unchanged trays and shelves.
        // The standard prepare/apply path repeats fit validation atomically.
        for shelf in &planned {
            for placement in &shelf.placements {
                let after = candidate
                    .placement(&placement.before.id)
                    .expect("existing placement is preserved");
                validation.issues.extend(
                    candidate
                        .validate_placement_location(
                            after,
                            products[placement.product_index],
                            shelf.shelf,
                            after.x,
                        )
                        .issues,
                );
            }
        }
        if !validation.valid() {
            return Err(CommandResult::ValidationFailed {
                revision: self.revision,
                validation,
            });
        }
        if changes.is_empty() {
            return Err(CommandResult::InvalidCommand {
                message: "Products already use this sales allocation for the selected contribution, target and facing bounds.".into(),
            });
        }

        let contributions = sales_basis_points(
            &weights
                .iter()
                .map(|weight| u128::from(*weight))
                .collect::<Vec<_>>(),
        );
        let before_shares = sales_basis_points(
            &rows
                .iter()
                .map(|row| match request.target {
                    SalesAllocationTarget::Space => row.before_space_sixteenths as u128,
                    SalesAllocationTarget::Facings => u128::from(row.before_facings),
                })
                .collect::<Vec<_>>(),
        );
        let after_shares = sales_basis_points(&measures);
        for (index, row) in rows.iter_mut().enumerate() {
            row.contribution_basis_points = contributions[index];
            row.before_share_basis_points = before_shares[index];
            row.after_share_basis_points = after_shares[index];
        }
        let zero_weight_sku_count = weights.iter().filter(|weight| **weight == 0).count();
        let mut warnings = vec![
            "This deterministic incremental allocation is a bounded heuristic; fixed shelf assignments, indivisible facings and facing bounds can prevent exact contribution shares.".into(),
            "Changing facings does not change assumed demand or predict sales uplift.".into(),
        ];
        if fixed_tray_count > 0 {
            warnings.push("Fixed tray presets are included in reported shares and retained unchanged; trays may be re-spaced on their existing shelf.".into());
        }
        if zero_weight_sku_count > 0 {
            warnings.push("Products with zero contribution keep the minimum facings in each loose placement; their fixed trays are retained.".into());
        }
        if loose_count + fixed_tray_count > products.len() {
            warnings.push("Demand is counted once per SKU across the scope; facing bounds apply to each existing loose placement separately.".into());
        }
        Ok((
            changes,
            SalesAllocationReport {
                basis: request.basis,
                target: request.target,
                scope: request.scope.clone(),
                product_count: products.len(),
                shelf_count: planned.len(),
                fixed_tray_count,
                zero_weight_sku_count,
                period: period.into(),
                source: source.into(),
                rows,
                warnings,
            },
        ))
    }

    #[allow(clippy::result_large_err)]
    fn checked_sales_width(
        &self,
        placement: &Placement,
        product: &Product,
    ) -> Result<Length, CommandResult> {
        // Protect the existing authoritative footprint helper before using it
        // on caller-provided catalogs. Width/height/depth stay in Length's range.
        if product.tray.is_none() {
            for (dimension, count, code) in [
                (
                    product.dimensions.width,
                    placement.facings_x,
                    ValidationCode::PlacementOutOfBounds,
                ),
                (
                    product.dimensions.height,
                    placement.facings_y,
                    ValidationCode::PlacementTooTall,
                ),
                (
                    product.dimensions.depth,
                    placement.facings_z,
                    ValidationCode::PlacementTooDeep,
                ),
            ] {
                if dimension.sixteenths().checked_mul(count as i32).is_none() {
                    return Err(self.sales_allocation_invalid(
                        code,
                        Some(placement.shelf_id.clone()),
                        "The placement footprint exceeds the supported geometry range.",
                    ));
                }
            }
        }
        Ok(Self::display_width(placement, product))
    }
}

/// Preserve the existing block distributor and generator behavior, while
/// checking exact grid-rounded packing in wider arithmetic before calling it.
fn sales_distribution(
    widths: &[Length],
    joined: &[bool],
    shelf_width: Length,
    distribution: ShelfDistribution,
) -> Option<Vec<Length>> {
    let mut next_x = 0_i64;
    for width in widths {
        if *width <= Length::ZERO {
            return None;
        }
        let end = next_x + i64::from(width.sixteenths());
        if end > i64::from(shelf_width.sixteenths()) {
            return None;
        }
        next_x = end + i64::from(MIN_PLACEMENT_GAP.sixteenths());
        next_x += next_x.rem_euclid(2);
        if next_x > i64::from(i32::MAX) {
            return None;
        }
    }
    resolve_block_distribution(widths, joined, shelf_width, distribution)
}

/// Largest-remainder rounding keeps displayed shares at exactly 100.00%.
/// Input order is stable product-ID order, including for equal remainders.
fn sales_basis_points(values: &[u128]) -> Vec<u32> {
    let total: u128 = values.iter().sum();
    if total == 0 {
        return vec![0; values.len()];
    }
    let mut points = values
        .iter()
        .map(|value| (value * 10_000 / total) as u32)
        .collect::<Vec<_>>();
    let mut indices = (0..values.len()).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        ((values[*right] * 10_000) % total)
            .cmp(&((values[*left] * 10_000) % total))
            .then_with(|| left.cmp(right))
    });
    let remainder = 10_000 - points.iter().sum::<u32>();
    for index in indices.into_iter().take(remainder as usize) {
        points[index] += 1;
    }
    points
}

fn sales_totals_overflow() -> CommandResult {
    CommandResult::InvalidCommand {
        message: "The selected scope exceeds the supported allocation totals.".into(),
    }
}

fn sales_source_family<'a>(source: &'a str, cereal_source: Option<&'a str>) -> &'a str {
    let source = source.trim();
    if let Some(cereal_source) = cereal_source {
        let valid_price = source
            .strip_prefix(cereal_source)
            .and_then(|suffix| suffix.strip_prefix("; illustrative unit price "))
            .and_then(|suffix| suffix.strip_suffix(" cents"))
            .filter(|price| !price.is_empty() && price.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|price| price.parse::<u64>().ok())
            .is_some_and(|price| price > 0);
        if valid_price {
            return cereal_source;
        }
    }
    source
}

fn sales_preview_error(result: CommandResult) -> PreviewResult {
    match result {
        CommandResult::ValidationFailed {
            revision,
            validation,
        } => PreviewResult::ValidationFailed {
            revision,
            validation,
        },
        CommandResult::RevisionConflict {
            expected_revision,
            current_revision,
        } => PreviewResult::RevisionConflict {
            expected_revision,
            current_revision,
        },
        CommandResult::NotFound { entity, id } => PreviewResult::NotFound { entity, id },
        CommandResult::Forbidden { message } => PreviewResult::Forbidden { message },
        CommandResult::InvalidCommand { message } => PreviewResult::InvalidCommand { message },
        CommandResult::Applied { .. } => PreviewResult::InvalidCommand {
            message: "The sales allocation could not be previewed.".into(),
        },
    }
}
