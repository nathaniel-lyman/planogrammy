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
