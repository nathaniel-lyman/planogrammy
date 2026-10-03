//! A bounded, reproducible simulation. These inputs describe fictitious products,
//! not retailer observations, demand forecasts, or a sales response to facings.
use super::*;
use std::collections::BTreeMap;

pub const CEREAL_GENERATOR_VERSION: u32 = 1;
const CEREAL_SHELF_ELEVATIONS_INCHES: [i32; 5] = [8, 23, 38, 53, 68];
const CEREAL_SOURCE: &str =
    "Seeded synthetic cereal assumptions; not retailer actuals or forecasts";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScenarioOrigin {
    pub seed: u32,
    pub bay_count: u32,
    pub generator_version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScenarioAssumptions {
    pub source: String,
    pub generator_version: u32,
    pub demand_period: String,
    pub layout_policy: String,
    pub capacity_formula: String,
    pub days_supply_formula: String,
    pub replenishment_formula: String,
    pub caveat: String,
}

pub fn cereal_assumptions() -> ScenarioAssumptions {
    ScenarioAssumptions {
        source: CEREAL_SOURCE.into(),
        generator_version: CEREAL_GENERATOR_VERSION,
        demand_period: "One simulated store-week; constant demand, seven days per week".into(),
        layout_policy: "100 SKUs: five fictional brands × five cereal families × four pack sizes. Five adjustable shelves per 4-foot bay; no stacking; front-facing boxes. Keep every SKU once, fill depth with whole units, then use Rust fill-evenly facing allocation and space-evenly spacing. Base decks remain empty.".into(),
        capacity_formula: "Sum Rust-derived stocked units over placements, grouped by SKU. Depth counts are floor(shelf depth / pack depth); a placement's capacity is facings_x × facings_y × facings_z.".into(),
        days_supply_formula: "Per SKU: capacity units × 7 / assumed weekly unit demand. Overall: total stocked capacity × 7 / stocked-SKU demand; this weighted stock-cover ratio is not a sum of SKU days and does not mean every SKU lasts that long. Values use millidays (1/1000 day).".into(),
        replenishment_formula: "Sum assumed weekly unit demand / capacity units across stocked SKUs. Report fractional full-shelf-equivalent turnovers per week in thousandths; not visits, cases, labor hours, or scheduled replenishment events. Missing SKUs are separately flagged and excluded from this proxy.".into(),
        caveat: "Illustrative inputs only. Brand, cereal family and pack size share correlated demand assumptions with bounded seeded variation. No substitution, lost-sales, service-level or financial forecast. More facings increase capacity, never assumed demand. No backroom stock, delivery schedule, case rounding or safety stock is modeled.".into(),
    }
}

/// Generate immutable version-1 genesis data. Changing this algorithm requires a
/// new generator version because snapshot history replays from this exact state.
pub fn cereal_draft(seed: u32, bay_count: u32) -> Result<DraftVersion, String> {
    if !matches!(bay_count, 6 | 8) {
        return Err(
            "The cereal challenge supports only an eight-bay baseline or six-bay alternative."
                .into(),
        );
    }
    let fixture_id = FixtureId::new("fixture_synthetic_cereal");
    let sections = (0..bay_count)
        .map(|sequence| {
            let section_id = SectionId::new(format!("bay_{:02}", sequence + 1));
            let mut shelves = vec![Shelf {
                id: ShelfId::new(format!("{}_base_deck", section_id.0)),
                section_id: section_id.clone(),
                kind: ShelfKind::BaseDeck,
                width: DEFAULT_FIXTURE_WIDTH,
                depth: DEFAULT_BASE_DECK_DEPTH,
                elevation: Length::ZERO,
            }];
            shelves.extend(CEREAL_SHELF_ELEVATIONS_INCHES.iter().enumerate().map(
                |(index, elevation)| Shelf {
                    id: ShelfId::new(format!("{}_shelf_{:02}", section_id.0, index + 1)),
                    section_id: section_id.clone(),
                    kind: ShelfKind::Adjustable,
                    width: DEFAULT_FIXTURE_WIDTH,
                    depth: DEFAULT_ADJUSTABLE_SHELF_DEPTH,
                    elevation: Length::inches(*elevation),
                },
            ));
            Section {
                id: section_id,
                fixture_id: fixture_id.clone(),
                sequence,
                width: DEFAULT_FIXTURE_WIDTH,
                height: DEFAULT_FIXTURE_HEIGHT,
                shelves,
            }
        })
        .collect();
    let mut draft = DraftVersion {
        id: VersionId::new(format!("cereal_{seed}_{bay_count}_genesis")),
        fixture: Fixture {
            id: fixture_id,
            name: format!("Synthetic cereal · {bay_count} bays"),
            width: Length::from_sixteenths(DEFAULT_FIXTURE_WIDTH.sixteenths() * bay_count as i32),
            height: DEFAULT_FIXTURE_HEIGHT,
            depth: DEFAULT_FIXTURE_DEPTH,
            sections,
        },
        products: cereal_products(seed),
        scenario_origin: Some(ScenarioOrigin {
            seed,
            bay_count,
            generator_version: CEREAL_GENERATOR_VERSION,
        }),
        ..DraftVersion::default()
    };
    let shelves = draft
        .fixture
        .sections
        .iter()
        .flat_map(|section| &section.shelves)
        .filter(|shelf| shelf.kind == ShelfKind::Adjustable)
        .cloned()
        .collect::<Vec<_>>();
    // Partition in stable brand/family/pack order. Each shelf gets two to four
    // SKUs, so the same full assortment fits before any surplus facings are added.
    for (index, shelf) in shelves.iter().enumerate() {
        let start = index * draft.products.len() / shelves.len();
        let end = (index + 1) * draft.products.len() / shelves.len();
        let mut cursor = Length::ZERO;
        for product in &draft.products[start..end] {
            let placement = Placement {
                id: PlacementId::new(format!("placement_{:04}", draft.next_placement)),
                product_id: product.id.clone(),
                shelf_id: shelf.id.clone(),
                x: cursor,
                facings_x: 1,
                facings_y: 1,
                facings_z: (shelf.depth.sixteenths() / product.dimensions.depth.sixteenths())
                    as u32,
            };
            cursor = align_to_eighth(
                cursor + DraftVersion::display_width(&placement, product) + MIN_PLACEMENT_GAP,
            );
            draft.placements.push(placement);
            draft.next_placement += 1;
        }
        // Reuse the semantic resolver; generation must not create an alternate
        // packing/facing implementation. Only genesis construction skips history.
        let allocation = draft
            .resolve_shelf_allocation(&shelf.id, ShelfAllocationStrategy::FillEvenly)
            .map_err(|_| {
                "The seeded assortment cannot fit the requested shelf layout.".to_string()
            })?;
        for change in allocation {
            let PlacementChange::Reflow {
                placement_id,
                shelf_id,
                resolved_x,
                facings_x,
                facings_y,
                facings_z,
            } = change
            else {
                return Err("Unexpected operation in generated shelf allocation.".into());
            };
            let placement = draft
                .placement_mut(&placement_id)
                .ok_or("Generated placement is missing.")?;
            placement.shelf_id = shelf_id;
            placement.x = resolved_x;
            placement.facings_x = facings_x;
            placement.facings_y = facings_y;
            placement.facings_z = facings_z;
        }
    }
    let validation = draft.validate_planogram();
    if !validation.valid {
        return Err(format!(
            "Generated cereal layout failed validation: {:?}",
            validation.validation.issues
        ));
    }
    Ok(draft)
}

fn cereal_products(seed: u32) -> Vec<Product> {
    // Brand demand is deliberately related to a price tier. Family and size
    // multipliers affect all corresponding products rather than random SKUs.
    let brands = [
        ("Fieldday", "value", 120_u64, 280_u64, [213, 156, 58]),
        ("Morning Mill", "mainstream", 110, 360, [197, 99, 69]),
        ("Suntrail", "mainstream", 100, 390, [96, 158, 189]),
        ("Hearth & Grain", "premium", 85, 470, [131, 115, 177]),
        ("Orchard Table", "organic", 70, 530, [104, 153, 103]),
    ];
    let families = [
        ("Honey Oat Rings", 125_u64),
        ("Toasted Corn Flakes", 115),
        ("Cocoa Rice Crunch", 105),
        ("Fruit & Bran", 80),
        ("Nutty Granola", 75),
    ];
    // (pack label, ounces, width/height/depth in sixteenths, demand %, price %)
    let packs = [
        ("Small", 10_u32, 96, 144, 32, 85_u64, 80_u64),
        ("Regular", 14, 112, 160, 36, 135, 100),
        ("Family", 20, 128, 184, 44, 110, 130),
        ("Value", 26, 144, 208, 52, 70, 160),
    ];
    let mut products = Vec::with_capacity(100);
    for (brand_index, (brand, tier, brand_demand, base_price, color)) in brands.iter().enumerate() {
        // Separate deterministic coordinates keep the bounded brand and family
        // factors stable regardless of iteration order or platform RNG versions.
        let brand_variation = 90 + seeded_value(seed, brand_index as u32, 0) % 21;
        for (family_index, (family, family_demand)) in families.iter().enumerate() {
            let family_variation = 95 + seeded_value(seed, family_index as u32, 1) % 11;
            for (pack_index, (pack, ounces, width, height, depth, pack_demand, price_factor)) in
                packs.iter().enumerate()
            {
                let units_milli = 18_000_u64
                    * brand_demand
                    * family_demand
                    * pack_demand
                    * u64::from(brand_variation)
                    * u64::from(family_variation)
                    / 10_000_000_000;
                let price_cents = base_price * price_factor / 100;
                let ordinal = products.len() + 1;
                products.push(Product {
                    id: ProductId::new(format!(
                        "cereal_{:02}_{:02}_{:02}",
                        brand_index + 1,
                        family_index + 1,
                        pack_index + 1
                    )),
                    upc: format!("SIM-CEREAL-{ordinal:04}"),
                    brand: (*brand).into(),
                    description: format!("{brand} {family} · {pack} {ounces} oz · {tier}"),
                    size_oz: ounces.to_string(),
                    category: "Synthetic cereal".into(),
                    dimensions: ProductDimensions {
                        width: Length::from_sixteenths(*width),
                        height: Length::from_sixteenths(*height),
                        depth: Length::from_sixteenths(*depth),
                        source:
                            "Simulated front-facing cereal boxes; correlated pack-size dimensions"
                                .into(),
                        confidence: "synthetic assumption".into(),
                    },
                    net_weight_ounces_hundredths: ounces * 100,
                    casepack_quantity: if pack_index < 2 { 12 } else { 6 },
                    performance: ProductPerformance {
                        sales_per_store_per_week_cents: units_milli * price_cents / 1_000,
                        units_per_store_per_week_milliunits: units_milli,
                        gross_margin_basis_points: 2_400 + brand_index as i32 * 150,
                        source: format!(
                            "{CEREAL_SOURCE}; illustrative unit price {price_cents} cents"
                        ),
                        period: "Synthetic steady-state weekly assumption".into(),
                    },
                    tray: None,
                    color: *color,
                    lid_color: [238, 226, 195],
                });
            }
        }
    }
    products
}

fn seeded_value(seed: u32, index: u32, stream: u32) -> u32 {
    let mut value = seed
        .wrapping_add(index.wrapping_mul(0x9e37_79b9))
        .wrapping_add(stream.wrapping_mul(0x85eb_ca6b));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScenarioMetrics {
    pub bay_count: u32,
    pub distinct_sku_count: u32,
    pub expected_sku_count: u32,
    pub unplaced_product_ids: Vec<ProductId>,
    pub capacity_units: u64,
    /// Includes missing assortment; each SKU's baseline demand is counted once.
    pub weekly_demand_milliunits: u64,
    pub stocked_weekly_demand_milliunits: u64,
    /// Weighted stock-cover ratio, never a sum of per-SKU days.
    pub aggregate_days_supply_millidays: Option<u64>,
    /// Sum of per-SKU demand/capacity, excluding missing SKUs (reported above).
    pub replenishment_turnovers_per_week_milli: u64,
    pub below_seven_days_sku_count: u32,
    pub validation_issue_count: u32,
    pub within_six_bay_limit: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScenarioProductComparison {
    pub product_id: ProductId,
    pub description: String,
    pub weekly_demand_milliunits: u64,
    pub baseline_capacity_units: u64,
    pub current_capacity_units: u64,
    pub baseline_days_supply_millidays: Option<u64>,
    pub current_days_supply_millidays: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScenarioComparison {
    pub assumptions: ScenarioAssumptions,
    pub baseline: ScenarioMetrics,
    pub current: ScenarioMetrics,
    pub products: Vec<ScenarioProductComparison>,
}

pub fn compare_cereal(baseline: &DraftVersion, current: &DraftVersion) -> ScenarioComparison {
    // Preserve the baseline assortment and demand denominator even after a SKU
    // is removed. Include additional catalog SKUs once, with stable ID ordering.
    let mut catalog = BTreeMap::new();
    for product in current.products.iter().chain(&baseline.products) {
        catalog.insert(product.id.clone(), product);
    }
    let baseline_capacity = baseline.stocked_units_by_product();
    let current_capacity = current.stocked_units_by_product();
    let products = catalog
        .values()
        .map(|product| {
            let before = baseline_capacity.get(&product.id).copied().unwrap_or(0);
            let after = current_capacity.get(&product.id).copied().unwrap_or(0);
            let demand = product.performance.units_per_store_per_week_milliunits;
            ScenarioProductComparison {
                product_id: product.id.clone(),
                description: product.description.clone(),
                weekly_demand_milliunits: demand,
                baseline_capacity_units: before,
                current_capacity_units: after,
                baseline_days_supply_millidays: days_supply(before, demand),
                current_days_supply_millidays: days_supply(after, demand),
            }
        })
        .collect();
    ScenarioComparison {
        assumptions: cereal_assumptions(),
        baseline: metrics(baseline, &catalog, &baseline_capacity),
        current: metrics(current, &catalog, &current_capacity),
        products,
    }
}

fn metrics(
    draft: &DraftVersion,
    catalog: &BTreeMap<ProductId, &Product>,
    capacities: &BTreeMap<ProductId, u64>,
) -> ScenarioMetrics {
    let mut unplaced = Vec::new();
    let mut capacity = 0_u64;
    let mut demand = 0_u64;
    let mut stocked_demand = 0_u64;
    let mut turnovers = 0_u64;
    let mut below_seven = 0;
    for (id, product) in catalog {
        let units = capacities.get(id).copied().unwrap_or(0);
        let weekly = product.performance.units_per_store_per_week_milliunits;
        demand = demand.saturating_add(weekly);
        capacity = capacity.saturating_add(units);
        if units == 0 {
            unplaced.push(id.clone());
        } else {
            stocked_demand = stocked_demand.saturating_add(weekly);
            turnovers = turnovers.saturating_add(weekly / units);
            if u128::from(units) * 1_000 < u128::from(weekly) {
                below_seven += 1;
            }
        }
    }
    ScenarioMetrics {
        bay_count: draft.fixture.sections.len() as u32,
        distinct_sku_count: (catalog.len() - unplaced.len()) as u32,
        expected_sku_count: catalog.len() as u32,
        unplaced_product_ids: unplaced,
        capacity_units: capacity,
        weekly_demand_milliunits: demand,
        stocked_weekly_demand_milliunits: stocked_demand,
        aggregate_days_supply_millidays: days_supply(capacity, stocked_demand),
        replenishment_turnovers_per_week_milli: turnovers,
        below_seven_days_sku_count: below_seven,
        validation_issue_count: draft.validate_planogram().validation.issues.len() as u32,
        within_six_bay_limit: draft.fixture.sections.len() <= 6
            && draft.fixture.width.sixteenths() <= DEFAULT_FIXTURE_WIDTH.sixteenths() * 6,
    }
}
