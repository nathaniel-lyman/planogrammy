mod bay_file;
mod document;
#[cfg(test)]
mod golden;
use document::EditorDocument;
use planogram_core::{
    ChangeSetId, CommandResult, DraftVersion, FacingsRequest, Length, PlacementChange, PlacementId,
    ProductId, SalesAllocationRequest, ShelfAllocationStrategy, ShelfDistribution, ShelfId,
    VersionId,
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

fn parse_shelf_allocation_strategy(value: &str) -> Option<ShelfAllocationStrategy> {
    match value {
        "fill_evenly" => Some(ShelfAllocationStrategy::FillEvenly),
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
    document: EditorDocument,
    document_revision: u32,
    renderer: Option<WebGpuRenderer>,
}

#[wasm_bindgen]
impl PlanogramEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            document: EditorDocument::Bay {
                draft: DraftVersion::default(),
            },
            document_revision: 0,
            renderer: None,
        }
    }

    pub async fn initialize_renderer(&mut self, canvas_id: String) -> Result<(), JsValue> {
        let renderer = WebGpuRenderer::new(&canvas_id, self.document.draft().render_scene())
            .await
            .map_err(|message| JsValue::from_str(&message))?;
        self.renderer = Some(renderer);
        Ok(())
    }

    pub fn export_bay(&self, name: String) -> Result<String, JsValue> {
        bay_file::export_document(&self.document, &name).map_err(|error| JsValue::from_str(&error))
    }

    pub fn inspect_bay(&self, json: String) -> Result<String, JsValue> {
        bay_file::parse_document(&json)
            .map(|(name, _)| name)
            .map_err(|error| JsValue::from_str(&error))
    }

    pub fn restore_bay(&mut self, json: String, expected_revision: u32) -> Result<String, JsValue> {
        if self.document_revision != expected_revision {
            return Err(JsValue::from_str(
                "The bay changed while opening the file. Try Open again.",
            ));
        }
        let (name, document) =
            bay_file::parse_document(&json).map_err(|error| JsValue::from_str(&error))?;
        let scene = document.draft().render_scene();
        self.document = document;
        self.document_revision += 1;
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.replace_scene(scene);
            let _ = renderer.render();
        }
        Ok(name)
    }

    pub fn start_cereal(
        &mut self,
        seed: u32,
        expected_document_revision: u32,
    ) -> Result<(), JsValue> {
        self.check_document_revision(expected_document_revision)?;
        self.document = EditorDocument::cereal(seed).map_err(|e| JsValue::from_str(&e))?;
        self.document_changed();
        Ok(())
    }
    pub fn select_alternative(
        &mut self,
        index: i32,
        expected_document_revision: u32,
    ) -> Result<(), JsValue> {
        self.check_document_revision(expected_document_revision)?;
        self.document
            .select(index)
            .map_err(|e| JsValue::from_str(&e))?;
        self.document_changed();
        Ok(())
    }
    pub fn duplicate_alternative(
        &mut self,
        expected_document_revision: u32,
    ) -> Result<(), JsValue> {
        self.check_document_revision(expected_document_revision)?;
        self.document
            .duplicate()
            .map_err(|e| JsValue::from_str(&e))?;
        self.document_changed();
        Ok(())
    }
    fn check_document_revision(&self, expected: u32) -> Result<(), JsValue> {
        if expected != self.document_revision {
            Err(JsValue::from_str("The document changed. Try again."))
        } else {
            Ok(())
        }
    }
    fn document_changed(&mut self) {
        self.document_revision += 1;
        let scene = self.document.draft().render_scene();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.replace_scene(scene);
            let _ = renderer.render();
        }
    }
    pub fn focus_bay(&mut self, shelf_id: String) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.model.focus_bay(&ShelfId::new(shelf_id));
            let _ = renderer.render();
        }
    }

    pub fn context(&self) -> Result<JsValue, JsValue> {
        #[derive(Serialize)]
        struct Context<'a> {
            document_revision: u32,
            scenario: Option<document::ScenarioView>,
            version_id: &'a str,
            version_status: planogram_core::VersionStatus,
            revision: u64,
            fixture: &'a planogram_core::Fixture,
            products: &'a [planogram_core::Product],
            placements: Vec<planogram_core::PlacementView>,
            latest_change_set_id: Option<&'a str>,
            latest_undoable_change_set_id: Option<&'a str>,
        }
        to_js(&Context {
            document_revision: self.document_revision,
            scenario: self.document.view(),
            version_id: &self.document.draft().id.0,
            version_status: self.document.draft().status,
            revision: self.document.draft().revision,
            fixture: &self.document.draft().fixture,
            products: &self.document.draft().products,
            placements: self.document.draft().placement_views(),
            latest_change_set_id: self
                .document
                .draft()
                .latest_change_set_id()
                .map(|id| id.0.as_str()),
            latest_undoable_change_set_id: self
                .document
                .draft()
                .latest_undoable_change_set_id()
                .map(|id| id.0.as_str()),
        })
    }

    pub fn validate_planogram(&self) -> Result<JsValue, JsValue> {
        to_js(&self.document.draft().validate_planogram())
    }

    pub fn add_placement(
        &mut self,
        version_id: String,
        product_id: String,
        shelf_id: String,
        expected_revision: u32,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.document.draft_mut().add_placement(
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
        let result = self.document.draft_mut().add_placement_as(
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
        let result = self.document.draft_mut().remove_placement(
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
        let result = self.document.draft_mut().move_shelf(
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
        let result = self.document.draft_mut().move_placement(
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

    #[allow(clippy::too_many_arguments)]
    pub fn set_facings(
        &mut self,
        version_id: String,
        placement_id: String,
        facings_x: Option<u32>,
        facings_y: Option<u32>,
        facings_z: Option<u32>,
        expected_revision: u32,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.document.draft_mut().set_facings(
            &VersionId::new(version_id),
            &PlacementId::new(placement_id),
            FacingsRequest {
                facings_x,
                facings_y,
                facings_z,
            },
            u64::from(expected_revision),
            reason,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_facings_as(
        &mut self,
        version_id: String,
        placement_id: String,
        facings_x: Option<u32>,
        facings_y: Option<u32>,
        facings_z: Option<u32>,
        expected_revision: u32,
        actor: String,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = self.document.draft_mut().set_facings_as(
            &VersionId::new(version_id),
            &PlacementId::new(placement_id),
            FacingsRequest {
                facings_x,
                facings_y,
                facings_z,
            },
            u64::from(expected_revision),
            actor,
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
            Some(distribution) => self.document.draft_mut().distribute_shelf(
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
            Some(distribution) => self.document.draft_mut().distribute_shelf_as(
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
        let result = self.document.draft().preview_placement_changes(
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

    pub fn preview_shelf_allocation(
        &mut self,
        version_id: String,
        shelf_id: String,
        strategy: String,
        expected_revision: u32,
    ) -> Result<JsValue, JsValue> {
        let result = match parse_shelf_allocation_strategy(&strategy) {
            Some(strategy) => self.document.draft().preview_shelf_allocation(
                &VersionId::new(version_id),
                &ShelfId::new(shelf_id),
                strategy,
                u64::from(expected_revision),
            ),
            None => planogram_core::PreviewResult::InvalidCommand {
                message: format!("Unknown shelf allocation strategy: {strategy}."),
            },
        };
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

    pub fn preview_sales_allocation(
        &mut self,
        version_id: String,
        request: JsValue,
        expected_revision: u32,
    ) -> Result<JsValue, JsValue> {
        let request: SalesAllocationRequest =
            serde_wasm_bindgen::from_value(request).map_err(|error| {
                JsValue::from_str(&format!("Invalid sales allocation request: {error}"))
            })?;
        let result = self.document.draft().preview_sales_allocation(
            &VersionId::new(version_id),
            &request,
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

    pub fn apply_changes_as(
        &mut self,
        version_id: String,
        expected_revision: u32,
        changes: JsValue,
        actor: String,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let changes = parse_placement_changes(changes)?;
        let result = self.document.draft_mut().apply_placement_changes_as(
            &VersionId::new(version_id),
            &changes,
            u64::from(expected_revision),
            actor,
            reason,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn apply_shelf_allocation_as(
        &mut self,
        version_id: String,
        shelf_id: String,
        strategy: String,
        expected_revision: u32,
        actor: String,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let result = match parse_shelf_allocation_strategy(&strategy) {
            Some(strategy) => self.document.draft_mut().apply_shelf_allocation_as(
                &VersionId::new(version_id),
                &ShelfId::new(shelf_id),
                strategy,
                u64::from(expected_revision),
                actor,
                reason,
            ),
            None => CommandResult::InvalidCommand {
                message: format!("Unknown shelf allocation strategy: {strategy}."),
            },
        };
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    pub fn apply_sales_allocation_as(
        &mut self,
        version_id: String,
        request: JsValue,
        expected_revision: u32,
        actor: String,
        reason: String,
    ) -> Result<JsValue, JsValue> {
        let request: SalesAllocationRequest =
            serde_wasm_bindgen::from_value(request).map_err(|error| {
                JsValue::from_str(&format!("Invalid sales allocation request: {error}"))
            })?;
        let result = self.document.draft_mut().apply_sales_allocation_as(
            &VersionId::new(version_id),
            &request,
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
        let result = self.document.draft_mut().undo_change_set(
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
        let result = self.document.draft_mut().undo_change_set_as(
            &VersionId::new(version_id),
            &ChangeSetId::new(change_set_id),
            u64::from(expected_revision),
            actor,
        );
        self.apply_result_to_renderer(&result);
        to_js(&result)
    }

    fn apply_result_to_renderer(&mut self, result: &CommandResult) {
        if matches!(result, CommandResult::Applied { .. }) {
            self.document_revision += 1;
        }
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

    /// Screen rectangles (CSS px) of placements at least `min_width` by
    /// `min_height` on screen, for the HTML label overlay.
    pub fn placement_labels(&self, min_width: f32, min_height: f32) -> Result<JsValue, JsValue> {
        to_js(
            &self
                .renderer
                .as_ref()
                .map(|renderer| renderer.model.placement_labels(min_width, min_height))
                .unwrap_or_default(),
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
