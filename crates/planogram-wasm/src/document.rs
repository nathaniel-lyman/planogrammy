use planogram_core::{
    cereal_assumptions, cereal_draft, compare_cereal, DraftVersion, ScenarioAssumptions,
    ScenarioComparison, VersionId, VersionStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    pub name: String,
    pub draft: DraftVersion,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EditorDocument {
    Bay {
        draft: DraftVersion,
    },
    Cereal {
        seed: u32,
        assumptions: ScenarioAssumptions,
        baseline: Box<DraftVersion>,
        alternatives: Vec<Alternative>,
        active: Option<usize>,
    },
}
#[derive(Serialize)]
pub struct ScenarioView {
    pub seed: u32,
    pub active: Option<usize>,
    pub alternatives: Vec<String>,
    pub comparison: ScenarioComparison,
}
impl EditorDocument {
    pub fn cereal(seed: u32) -> Result<Self, String> {
        let mut baseline = cereal_draft(seed, 8)?;
        baseline.id = VersionId::new("cereal_baseline");
        baseline.status = VersionStatus::Published;
        let mut target = cereal_draft(seed, 6)?;
        target.id = VersionId::new("cereal_alternative_1");
        Ok(Self::Cereal {
            seed,
            assumptions: cereal_assumptions(),
            baseline: Box::new(baseline),
            alternatives: vec![Alternative {
                name: "Six-bay target".into(),
                draft: target,
            }],
            active: Some(0),
        })
    }
    pub fn draft(&self) -> &DraftVersion {
        match self {
            Self::Bay { draft } => draft,
            Self::Cereal {
                baseline,
                alternatives,
                active,
                ..
            } => active.map(|i| &alternatives[i].draft).unwrap_or(baseline),
        }
    }
    pub fn draft_mut(&mut self) -> &mut DraftVersion {
        match self {
            Self::Bay { draft } => draft,
            Self::Cereal {
                baseline,
                alternatives,
                active,
                ..
            } => active
                .map(|i| &mut alternatives[i].draft)
                .unwrap_or(baseline),
        }
    }
    pub fn view(&self) -> Option<ScenarioView> {
        match self {
            Self::Bay { .. } => None,
            Self::Cereal {
                seed,
                baseline,
                alternatives,
                active,
                ..
            } => Some(ScenarioView {
                seed: *seed,
                active: *active,
                alternatives: alternatives.iter().map(|a| a.name.clone()).collect(),
                comparison: compare_cereal(baseline, self.draft()),
            }),
        }
    }
    pub fn select(&mut self, index: i32) -> Result<(), String> {
        match self {
            Self::Cereal {
                alternatives,
                active,
                ..
            } if index >= -1 && index < alternatives.len() as i32 => {
                *active = usize::try_from(index).ok();
                Ok(())
            }
            _ => Err("Unknown alternative.".into()),
        }
    }
    pub fn duplicate(&mut self) -> Result<(), String> {
        match self {
            Self::Cereal {
                alternatives,
                active,
                ..
            } if alternatives.len() < 6 => {
                let mut draft = alternatives[active.unwrap_or(0)].draft.clone();
                draft.id = VersionId::new(format!("cereal_alternative_{}", alternatives.len() + 1));
                alternatives.push(Alternative {
                    name: format!("Alternative {}", alternatives.len() + 1),
                    draft,
                });
                *active = Some(alternatives.len() - 1);
                Ok(())
            }
            _ => Err("Up to six editable alternatives are supported.".into()),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Bay { draft } => {
                if draft.scenario_origin.is_some() {
                    return Err("Scenario requires its baseline.".into());
                }
                draft.validate_snapshot()
            }
            Self::Cereal {
                seed,
                assumptions,
                baseline,
                alternatives,
                active,
            } => {
                let Self::Cereal {
                    baseline: expected, ..
                } = Self::cereal(*seed)?
                else {
                    unreachable!()
                };
                if *assumptions != cereal_assumptions()
                    || *baseline != expected
                    || alternatives.is_empty()
                    || alternatives.len() > 6
                    || active.is_some_and(|i| i >= alternatives.len())
                {
                    return Err("Inconsistent cereal scenario or baseline.".into());
                }
                for (i, alternative) in alternatives.iter().enumerate() {
                    if alternative.name.is_empty()
                        || alternative.name.len() > 100
                        || alternative.draft.id.0 != format!("cereal_alternative_{}", i + 1)
                        || !alternative
                            .draft
                            .scenario_origin
                            .as_ref()
                            .is_some_and(|o| o.seed == *seed && o.bay_count == 6)
                    {
                        return Err("Invalid six-bay alternative.".into());
                    }
                    alternative.draft.validate_snapshot()?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bay_file::{export_document, parse_document};
    use planogram_core::{
        CommandResult, PreviewResult, ProductId, SalesAllocationBasis, SalesAllocationRequest,
        SalesAllocationScope, SalesAllocationTarget, ShelfId,
    };

    fn allocation_round_trip(mut document: EditorDocument, request: SalesAllocationRequest) {
        let before = document.clone();
        let version = document.draft().id.clone();
        let revision = document.draft().revision;
        let before_json = export_document(&document, "Synthetic allocation").unwrap();
        let preview = document
            .draft()
            .preview_sales_allocation(&version, &request, revision);
        let PreviewResult::Ready {
            operations,
            sales_allocation: Some(report),
            preview_scene,
            ..
        } = preview
        else {
            panic!("allocation preview must be ready: {preview:?}");
        };
        assert!(!report.rows.is_empty());
        assert!(report.source.to_ascii_lowercase().contains("synthetic"));
        assert!(!report.period.is_empty());
        // Saving a preview serializes exactly the pre-preview committed document.
        assert_eq!(document, before);
        assert_eq!(
            export_document(&document, "Synthetic allocation").unwrap(),
            before_json
        );
        let result = document.draft_mut().apply_sales_allocation_as(
            &version,
            &request,
            revision,
            "human",
            "Review synthetic sales allocation",
        );
        let CommandResult::Applied { change_set, .. } = result else {
            panic!("allocation apply must succeed: {result:?}");
        };
        assert_eq!(change_set.operations, operations);
        assert_eq!(change_set.actor, "human");
        assert_eq!(document.draft().revision, revision + 1);
        assert_eq!(
            document.draft().render_scene().placements,
            preview_scene.placements
        );
        assert_eq!(document.draft().products, before.draft().products);
        if let (Some(before_view), Some(after_view)) = (before.view(), document.view()) {
            assert_eq!(
                before_view.comparison.current.weekly_demand_milliunits,
                after_view.comparison.current.weekly_demand_milliunits
            );
            assert_eq!(
                before_view
                    .comparison
                    .current
                    .stocked_weekly_demand_milliunits,
                after_view
                    .comparison
                    .current
                    .stocked_weekly_demand_milliunits
            );
            assert_eq!(
                before_view.comparison.baseline,
                after_view.comparison.baseline
            );
        }
        let saved = export_document(&document, "Synthetic allocation").unwrap();
        let (_, mut opened) = parse_document(&saved).unwrap();
        assert_eq!(opened, document);
        assert!(matches!(
            opened
                .draft_mut()
                .undo_change_set(&version, &change_set.id, revision + 1),
            CommandResult::Applied { .. }
        ));
        assert_eq!(opened.draft().revision, revision + 2);
        assert_eq!(opened.draft().placements, before.draft().placements);
        assert_eq!(opened.draft().products, before.draft().products);
        assert_eq!(
            parse_document(&export_document(&opened, "Undo allocation").unwrap())
                .unwrap()
                .1,
            opened
        );
    }

    #[test]
    fn standard_sales_allocation_preview_commit_save_open_and_undo() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        for product in ["jif_creamy_16", "jif_crunchy_16", "skippy_creamy_40"] {
            let revision = draft.revision;
            assert!(matches!(
                draft.add_placement(
                    &version,
                    &ProductId::new(product),
                    &ShelfId::new("shelf_01"),
                    revision,
                    "Synthetic demo assortment"
                ),
                CommandResult::Applied { .. }
            ));
        }
        allocation_round_trip(
            EditorDocument::Bay { draft },
            SalesAllocationRequest {
                scope: SalesAllocationScope::Shelf {
                    shelf_id: ShelfId::new("shelf_01"),
                },
                basis: SalesAllocationBasis::Revenue,
                target: SalesAllocationTarget::Space,
                min_facings: 1,
                max_facings: 24,
            },
        );
    }

    #[test]
    fn cereal_sales_allocation_round_trip_preserves_baseline_and_assumed_demand() {
        let document = EditorDocument::cereal(20260930).unwrap();
        let section_id = document.draft().fixture.sections[0].id.clone();
        allocation_round_trip(
            document,
            SalesAllocationRequest {
                scope: SalesAllocationScope::Bay { section_id },
                basis: SalesAllocationBasis::Units,
                target: SalesAllocationTarget::Facings,
                min_facings: 1,
                max_facings: 24,
            },
        );
    }

    #[test]
    fn sales_request_transport_rejects_unknown_and_malformed_nested_fields() {
        let request = serde_json::json!({
            "scope": { "kind": "shelf", "shelf_id": "shelf_01" },
            "basis": "revenue", "target": "space", "min_facings": 1, "max_facings": 24
        });
        assert!(serde_json::from_value::<SalesAllocationRequest>(request.clone()).is_ok());
        for (key, value) in [
            (
                "scope",
                serde_json::json!({ "kind": "shelf", "shelf_id": "shelf_01", "x_sixteenths": 0 }),
            ),
            (
                "scope",
                serde_json::json!({ "kind": "bay", "section_id": "section_01", "shelf_id": "shelf_01" }),
            ),
            (
                "scope",
                serde_json::json!({ "kind": "shelf", "shelf_id": { "nested": true } }),
            ),
            ("basis", serde_json::json!("profit")),
            ("target", serde_json::json!("uplift")),
            ("max_facings", serde_json::json!(-1)),
            ("min_facings", serde_json::json!(1.5)),
            ("coordinates", serde_json::json!([0])),
        ] {
            let mut invalid = request.clone();
            invalid[key] = value;
            assert!(
                serde_json::from_value::<SalesAllocationRequest>(invalid).is_err(),
                "{key}"
            );
        }
    }

    #[test]
    fn scenario_round_trip_preserves_alternatives_baseline_and_history() {
        let mut doc = EditorDocument::cereal(20260930).unwrap();
        let before = doc.draft().placements.clone();
        let draft = doc.draft_mut();
        let version = draft.id.clone();
        let id = draft.placements[0].id.clone();
        assert!(matches!(
            draft.remove_placement(&version, &id, 0, "Remove for alternative"),
            planogram_core::CommandResult::Applied { .. }
        ));
        doc.duplicate().unwrap();
        doc.select(-1).unwrap();
        assert_eq!(doc.draft().status, VersionStatus::Published);
        let json = export_document(&doc, "Cereal").unwrap();
        let (_, mut opened) = parse_document(&json).unwrap();
        assert_eq!(opened, doc);
        opened.select(0).unwrap();
        let draft = opened.draft_mut();
        let version = draft.id.clone();
        let change = draft.latest_undoable_change_set_id().unwrap().clone();
        assert!(matches!(
            draft.undo_change_set(&version, &change, 1),
            planogram_core::CommandResult::Applied { .. }
        ));
        let mut restored = draft.placements.clone();
        restored.sort_by_key(|p| p.id.clone());
        assert_eq!(restored, before);
        assert_eq!(
            parse_document(&export_document(&opened, "Again").unwrap())
                .unwrap()
                .1,
            opened
        );
        let mut invalid: serde_json::Value = serde_json::from_str(&json).unwrap();
        invalid["document"]["baseline"]["revision"] = 1.into();
        assert!(parse_document(&invalid.to_string()).is_err());
        invalid = serde_json::from_str(&json).unwrap();
        invalid["document"]["alternatives"][0]["draft"]["scenario_origin"]["bay_count"] = 8.into();
        assert!(parse_document(&invalid.to_string()).is_err());
        invalid = serde_json::from_str(&json).unwrap();
        invalid["document"]["assumptions"]["source"] = "actual sales".into();
        assert!(parse_document(&invalid.to_string()).is_err());
    }
}
