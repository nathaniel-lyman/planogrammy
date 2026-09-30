//! Validates a committed draft by replaying its audit trail through the existing
//! command engine. This is independent of any file or browser representation.
use super::*;

impl DraftVersion {
    pub fn validate_snapshot(&self) -> Result<(), String> {
        let invalid = || "The bay snapshot or its change history is inconsistent.".to_string();
        let mut genesis = match &self.scenario_origin {
            Some(origin) if origin.generator_version == 1 => {
                cereal_draft(origin.seed, origin.bay_count)?
            }
            Some(_) => return Err("Unsupported scenario generator version.".into()),
            None => DraftVersion::default(),
        };
        let mut structure = self.fixture.clone();
        let initial = genesis.fixture.clone();
        if structure.sections.len() != initial.sections.len() {
            return Err(invalid());
        }
        for (section, original_section) in structure.sections.iter_mut().zip(&initial.sections) {
            if section.shelves.len() != original_section.shelves.len() {
                return Err(invalid());
            }
            for (shelf, original) in section.shelves.iter_mut().zip(&original_section.shelves) {
                shelf.elevation = original.elevation;
            }
        }
        if structure != initial
            || self.status != VersionStatus::Draft
            || self.id.0.is_empty()
            || self.change_sets.len() > 10_000
            || self.products.len() > 1_000
            || self.revision != self.change_sets.len() as u64
            || self.next_change_set != self.revision + 1
        {
            return Err(invalid());
        }
        // Bound geometry before it reaches the normal integer footprint math.
        // 1,000 inches leaves ample room for this bay while ensuring products
        // times the maximum 100 facings cannot overflow a signed Length.
        for product in &self.products {
            let dimensions = &product.dimensions;
            if product.id.0.is_empty()
                || [dimensions.width, dimensions.height, dimensions.depth]
                    .iter()
                    .any(|length| !(1..=16_000).contains(&length.sixteenths()))
                || product.tray.as_ref().is_some_and(|tray| {
                    [
                        tray.outer_width,
                        tray.outer_height,
                        tray.outer_depth,
                        tray.front_lip_height,
                    ]
                    .iter()
                    .any(|length| !(1..=16_000).contains(&length.sixteenths()))
                })
                || product.performance.sales_per_store_per_week_cents > 9_007_199_254_740_991
                || product.performance.units_per_store_per_week_milliunits > 9_007_199_254_740_991
            {
                return Err(invalid());
            }
        }
        genesis.id = self.id.clone();
        if self.scenario_origin.is_none() {
            genesis.products = self.products.clone();
        }
        let mut replay = genesis;
        if !replay.validate_planogram().valid {
            return Err(invalid());
        }
        let total = self.change_sets.len();
        // Name the first failing change set so an unopenable file can be traced
        // to the command whose semantics changed or whose record was edited.
        let failed = |index: usize, recorded: &ChangeSet, detail: &str| {
            format!(
                "The bay snapshot or its change history is inconsistent: change set {} ({} of {total}) {detail}.",
                recorded.id.0,
                index + 1
            )
        };
        for (index, recorded) in self.change_sets.iter().enumerate() {
            if recorded.operations.is_empty() || recorded.operations.len() > 1_000 {
                return Err(failed(index, recorded, "has an invalid operation count"));
            }
            let result = if let Some(compensates) = &recorded.compensates {
                replay.undo_change_set_as(&self.id, compensates, replay.revision, &recorded.actor)
            } else if let [PlanogramOperation::MoveShelf(movement)] = recorded.operations.as_slice()
            {
                if !(1..=DEFAULT_FIXTURE_HEIGHT.sixteenths()).contains(&movement.after.sixteenths())
                {
                    return Err(failed(index, recorded, "moves a shelf outside the fixture"));
                }
                replay.move_shelf(
                    &self.id,
                    &movement.shelf_id,
                    movement.after,
                    replay.revision,
                    &recorded.reason,
                )
            } else {
                let changes = recorded
                    .operations
                    .iter()
                    .map(snapshot_change)
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| failed(index, recorded, "contains an unreplayable operation"))?;
                // Each operation contains an exact resolved position, so replay
                // reuses the same atomic validator as undo and proposals.
                // Recorded layout: replay must reproduce each saved position
                // exactly, even if default shelf spacing changes later.
                replay.apply_placement_changes_with_compensation(
                    &self.id,
                    &changes,
                    replay.revision,
                    ChangeSetMetadata {
                        actor: recorded.actor.clone(),
                        reason: recorded.reason.clone(),
                        compensates: None,
                    },
                    ShelfLayout::Recorded,
                )
            };
            match result {
                CommandResult::Applied { change_set, .. } if change_set == *recorded => {}
                CommandResult::Applied { .. } => {
                    return Err(failed(
                        index,
                        recorded,
                        "replays to different operations than recorded",
                    ))
                }
                _ => return Err(failed(index, recorded, "is rejected by the current engine")),
            }
        }
        if replay != *self {
            return Err(
                "The bay snapshot or its change history is inconsistent: the replayed history does not match the saved snapshot."
                    .into(),
            );
        }
        Ok(())
    }
}

fn snapshot_change(operation: &PlanogramOperation) -> Option<PlacementChange> {
    let safe_x = |x: Length| (0..=16_000).contains(&x.sixteenths());
    match operation {
        PlanogramOperation::AddPlacement(add) if !safe_x(add.placement.x) => return None,
        PlanogramOperation::MovePlacement(movement) if !safe_x(movement.after.x) => return None,
        PlanogramOperation::ReflowPlacement(reflow) if !safe_x(reflow.after.x) => return None,
        _ => {}
    }
    Some(match operation {
        PlanogramOperation::MoveShelf(_) => return None,
        PlanogramOperation::AddPlacement(add) => PlacementChange::Add {
            placement_id: None,
            product_id: add.placement.product_id.clone(),
            shelf_id: add.placement.shelf_id.clone(),
            sequence: 0,
            resolved_x: Some(add.placement.x),
            facings_x: Some(add.placement.facings_x),
            facings_y: Some(add.placement.facings_y),
            facings_z: Some(add.placement.facings_z),
        },
        PlanogramOperation::RemovePlacement(remove) => PlacementChange::Remove {
            placement_id: remove.placement.id.clone(),
        },
        PlanogramOperation::MovePlacement(movement) => PlacementChange::Move {
            placement_id: movement.placement_id.clone(),
            shelf_id: movement.after.shelf_id.clone(),
            sequence: 0,
            resolved_x: Some(movement.after.x),
        },
        PlanogramOperation::ChangeFacings(change) => PlacementChange::SetFacings {
            placement_id: change.placement_id.clone(),
            facings: change.after,
        },
        PlanogramOperation::ReflowPlacement(reflow) => PlacementChange::Reflow {
            placement_id: reflow.placement_id.clone(),
            shelf_id: reflow.after.shelf_id.clone(),
            resolved_x: reflow.after.x,
            facings_x: reflow.after.facings_x,
            facings_y: reflow.after.facings_y,
            facings_z: reflow.after.facings_z,
        },
    })
}
