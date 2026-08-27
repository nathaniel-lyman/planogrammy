use planogram_core::{
    ChangeSetId, CommandResult, DraftVersion, Length, PlacementChange, PlacementId, ProductId,
    ShelfDistribution, ShelfId, VersionId,
};
use planogram_render::{Selection, WebGpuRenderer};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PlacementChangeInput {
    Add {
        product_id: String,
        shelf_id: String,
        sequence: u32,
        facings_x: Option<u32>,
        facings_y: Option<u32>,
        facings_z: Option<u32>,
    },
    Move {
        placement_id: String,
        shelf_id: String,
        sequence: u32,
    },
    Remove {
        placement_id: String,
    },
}

fn parse_shelf_distribution(value: &str) -> Option<ShelfDistribution> {
    match value {
        "packed_left" => Some(ShelfDistribution::PackedLeft),
        "centered" => Some(ShelfDistribution::Centered),
        "space_between" => Some(ShelfDistribution::SpaceBetween),
        "space_evenly" => Some(ShelfDistribution::SpaceEvenly),
        _ => None,
    }
}

fn parse_placement_changes(value: JsValue) -> Result<Vec<PlacementChange>, JsValue> {
    let inputs: Vec<PlacementChangeInput> = serde_wasm_bindgen::from_value(value)
        .map_err(|error| JsValue::from_str(&format!("Invalid placement changes: {error}")))?;
    Ok(inputs
        .into_iter()
        .map(|input| match input {
            PlacementChangeInput::Add {
                product_id,
                shelf_id,
                sequence,
                facings_x,
                facings_y,
                facings_z,
            } => PlacementChange::Add {
                placement_id: None,
                product_id: ProductId::new(product_id),
                shelf_id: ShelfId::new(shelf_id),
                sequence,
                resolved_x: None,
                facings_x,
                facings_y,
                facings_z,
            },
            PlacementChangeInput::Move {
                placement_id,
                shelf_id,
                sequence,
            } => PlacementChange::Move {
                placement_id: PlacementId::new(placement_id),
                shelf_id: ShelfId::new(shelf_id),
                sequence,
                resolved_x: None,
            },
            PlacementChangeInput::Remove { placement_id } => PlacementChange::Remove {
                placement_id: PlacementId::new(placement_id),
            },
        })
        .collect())
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(
            &serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(false),
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub struct PlanogramEngine {
    draft: DraftVersion,
    renderer: Option<WebGpuRenderer>,
}

#[wasm_bindgen]
impl PlanogramEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            draft: DraftVersion::default(),
            renderer: None,
        }
    }

    pub async fn initialize_renderer(&mut self, canvas_id: String) -> Result<(), JsValue> {
        let renderer = WebGpuRenderer::new(&canvas_id, self.draft.render_scene())
            .await
            .map_err(|message| JsValue::from_str(&message))?;
        self.renderer = Some(renderer);
        Ok(())
    }

    pub fn context(&self) -> Result<JsValue, JsValue> {
        #[derive(Serialize)]
        struct Context<'a> {
            version_id: &'a str,
            version_status: planogram_core::VersionStatus,
            revision: u64,
            fixture: &'a planogram_core::Fixture,
            products: &'a [planogram_core::Product],
            placements: Vec<planogram_core::PlacementView>,
            latest_change_set_id: Option<&'a str>,
        }
        to_js(&Context {
            version_id: &self.draft.id.0,
            version_status: self.draft.status,
            revision: self.draft.revision,
            fixture: &self.draft.fixture,
            products: &self.draft.products,
            placements: self.draft.placement_views(),
            latest_change_set_id: self.draft.latest_change_set_id().map(|id| id.0.as_str()),
        })
    }

    pub fn validate_planogram(&self) -> Result<JsValue, JsValue> {
        to_js(&self.draft.validate_planogram())
    }

    pub fn add_placement(
        &mut self,
        version_id: String,
        product_id: String,
        shelf_id: String,
        expected_revision: u32,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.draft.add_placement(
            &VersionId::new(version_id),
            &ProductId::new(product_id),
            &ShelfId::new(shelf_id),
            u64::from(expected_revision),
            reason,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn add_placement_as(
        &mut self,
        version_id: String,
        product_id: String,
        shelf_id: String,
        expected_revision: u32,
        actor: String,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.draft.add_placement_as(
            &VersionId::new(version_id),
            &ProductId::new(product_id),
            &ShelfId::new(shelf_id),
            u64::from(expected_revision),
            actor,
            reason,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn remove_placement(
        &mut self,
        version_id: String,
        placement_id: String,
        expected_revision: u32,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.draft.remove_placement(
            &VersionId::new(version_id),
            &PlacementId::new(placement_id),
            u64::from(expected_revision),
            reason,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn move_shelf(
        &mut self,
        version_id: String,
        shelf_id: String,
        elevation_sixteenths: i32,
        expected_revision: u32,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.draft.move_shelf(
            &VersionId::new(version_id),
            &ShelfId::new(shelf_id),
            Length::from_sixteenths(elevation_sixteenths),
            u64::from(expected_revision),
            reason,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn move_placement(
        &mut self,
        version_id: String,
        placement_id: String,
        target_shelf_id: String,
        x_sixteenths: i32,
        expected_revision: u32,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.draft.move_placement(
            &VersionId::new(version_id),
            &PlacementId::new(placement_id),
            &ShelfId::new(target_shelf_id),
            Length::from_sixteenths(x_sixteenths),
            u64::from(expected_revision),
            reason,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn distribute_shelf(
        &mut self,
        version_id: String,
        shelf_id: String,
        distribution: String,
        expected_revision: u32,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = match parse_shelf_distribution(&distribution) {
            Some(distribution) => self.draft.distribute_shelf(
                &VersionId::new(version_id),
                &ShelfId::new(shelf_id),
                distribution,
                u64::from(expected_revision),
                reason,
            ),
            None => CommandResult::InvalidCommand {
                message: format!("Unknown shelf distribution: {distribution}."),
            },
        };
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn distribute_shelf_as(
        &mut self,
        version_id: String,
        shelf_id: String,
        distribution: String,
        expected_revision: u32,
        actor: String,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = match parse_shelf_distribution(&distribution) {
            Some(distribution) => self.draft.distribute_shelf_as(
                &VersionId::new(version_id),
                &ShelfId::new(shelf_id),
                distribution,
                u64::from(expected_revision),
                actor,
                reason,
            ),
            None => CommandResult::InvalidCommand {
                message: format!("Unknown shelf distribution: {distribution}."),
            },
        };
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn preview_changes(
        &mut self,
        version_id: String,
        expected_revision: u32,
        changes: JsValue,
    ) -> Result<JsValue, JsValue> {
        let changes = parse_placement_changes(changes)?;
        let result = self.draft.preview_placement_changes(
            &VersionId::new(version_id),
            &changes,
            u64::from(expected_revision),
        );
        if let Some(renderer) = self.renderer.as_mut() {
            match &result {
                planogram_core::PreviewResult::Ready {
                    preview_scene,
                    affected_ids,
                    ..
                } => renderer
                    .model
                    .show_proposal_preview((**preview_scene).clone(), affected_ids.clone()),
                _ => renderer.model.clear_proposal_preview(),
            }
            let _ = renderer.render();
        }
        to_js(&result)
    }

    pub fn clear_proposal_preview(&mut self) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.clear_proposal_preview();
            let _ = renderer.render();
        }
    }

    pub fn apply_changes_as(
        &mut self,
        version_id: String,
        expected_revision: u32,
        changes: JsValue,
        actor: String,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let changes = parse_placement_changes(changes)?;
        let result = self.draft.apply_placement_changes_as(
            &VersionId::new(version_id),
            &changes,
            u64::from(expected_revision),
            actor,
            reason,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn undo_change_set(
        &mut self,
        version_id: String,
        change_set_id: String,
        expected_revision: u32,
    ) -> Result<JsValue, JsValue> {
        let result = self.draft.undo_change_set(
            &VersionId::new(version_id),
            &ChangeSetId::new(change_set_id),
            u64::from(expected_revision),
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn undo_change_set_as(
        &mut self,
        version_id: String,
        change_set_id: String,
        expected_revision: u32,
        actor: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.draft.undo_change_set_as(
            &VersionId::new(version_id),
            &ChangeSetId::new(change_set_id),
            u64::from(expected_revision),
            actor,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    fn apply_result_to_renderer(&mut self, result: &CommandResult) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        match result {
            CommandResult::Applied { scene_patch, .. } => renderer.model.apply_patch(scene_patch),
            CommandResult::ValidationFailed { validation, .. } => {
                renderer.model.validation_error =
                    validation.issues.first().map(|issue| issue.message.clone())
            }
            CommandResult::RevisionConflict { .. } => {
                renderer.model.validation_error =
                    Some("The planogram changed. Refresh before retrying.".into())
            }
            CommandResult::NotFound { .. }
            | CommandResult::Forbidden { .. }
            | CommandResult::InvalidCommand { .. } => {}
        }
        let _ = renderer.render();
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), JsValue> {
        self.renderer
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer not initialized"))?
            .resize(width, height)
            .map_err(|message| JsValue::from_str(&message))
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Result<JsValue, JsValue> {
        to_js(
            &self
                .renderer
                .as_ref()
                .and_then(|renderer| renderer.model.hit_test(x, y)),
        )
    }

    pub fn select_shelf(&mut self, shelf_id: String) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.select(Some(Selection::Shelf {
                id: ShelfId::new(shelf_id),
            }));
            let _ = renderer.render();
        }
    }

    pub fn select_placement(&mut self, placement_id: String) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.select(Some(Selection::Placement {
                id: PlacementId::new(placement_id),
            }));
            let _ = renderer.render();
        }
    }

    pub fn clear_selection(&mut self) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.select(None);
            let _ = renderer.render();
        }
    }

    pub fn begin_drag(&mut self, shelf_id: String, pointer_y: f32) -> bool {
        let Some(renderer) = self.renderer.as_mut() else {
            return false;
        };
        let started = renderer
            .model
            .begin_drag(&ShelfId::new(shelf_id), pointer_y);
        let _ = renderer.render();
        started
    }

    pub fn preview_drag(&mut self, pointer_y: f32) -> Option<i32> {
        let renderer = self.renderer.as_mut()?;
        let elevation = renderer
            .model
            .preview_drag(pointer_y)
            .map(Length::sixteenths);
        let _ = renderer.render();
        elevation
    }

    pub fn finish_drag(&mut self) -> Result<JsValue, JsValue> {
        let result = self
            .renderer
            .as_mut()
            .and_then(|renderer| renderer.model.finish_drag())
            .map(|(id, elevation)| (id.0, elevation.sixteenths()));
        to_js(&result)
    }

    pub fn cancel_drag(&mut self) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.cancel_drag();
            let _ = renderer.render();
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.zoom_by(factor);
            let _ = renderer.render();
        }
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.pan_by(dx, dy);
            let _ = renderer.render();
        }
    }

    pub fn fit_fixture(&mut self) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.fit();
            let _ = renderer.render();
        }
    }
}

impl Default for PlanogramEngine {
    fn default() -> Self {
        Self::new()
    }
}
