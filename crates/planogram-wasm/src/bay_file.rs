//! Version 1 portable bay envelope. JSON stays at the transport boundary;
//! history and physical validation remain in the domain engine.
use planogram_core::DraftVersion;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BayFile {
    format: String,
    format_version: u32,
    name: String,
    draft: Value,
}

pub fn export(draft: &DraftVersion, name: &str) -> Result<String, String> {
    validate_name(name)?;
    draft.validate_snapshot()?;
    let mut value = serde_json::to_value(draft).map_err(|error| error.to_string())?;
    geometry_keys(&mut value, true)?;
    let json = serde_json::to_string_pretty(&BayFile {
        format: "planogrammy-bay".into(),
        format_version: 1,
        name: name.to_string(),
        draft: value,
    })
    .map_err(|error| error.to_string())?;
    if json.len() > MAX_FILE_BYTES {
        return Err("This bay exceeds the 8 MiB file limit.".into());
    }
    Ok(json)
}

pub fn parse(json: &str) -> Result<(String, DraftVersion), String> {
    if json.len() > MAX_FILE_BYTES {
        return Err("Bay files must be 8 MiB or smaller.".into());
    }
    let mut file: BayFile =
        serde_json::from_str(json).map_err(|_| "This is not a valid Planogrammy bay file.")?;
    if file.format != "planogrammy-bay" || file.format_version != 1 {
        return Err("Unsupported bay file format or version. This editor opens version 1.".into());
    }
    validate_name(&file.name)?;
    geometry_keys(&mut file.draft, false)?;
    let draft: DraftVersion = serde_json::from_value(file.draft.clone())
        .map_err(|_| "The bay file contains malformed draft data.")?;
    // A canonical equality check rejects unknown fields and missing optional
    // fields, which serde's domain compatibility rules otherwise allow.
    if serde_json::to_value(&draft).map_err(|error| error.to_string())? != file.draft {
        return Err("The bay file contains unrecognized or incomplete draft data.".into());
    }
    draft.validate_snapshot()?;
    Ok((file.name, draft))
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err("Give the bay a name of 1–100 characters.".into());
    }
    Ok(())
}

// Domain serialization is also used by existing UI queries. The file boundary
// explicitly labels every Length without changing that established interface.
fn geometry_keys(value: &mut Value, exporting: bool) -> Result<(), String> {
    match value {
        Value::Array(values) => {
            for value in values {
                geometry_keys(value, exporting)?;
            }
        }
        Value::Object(fields) => {
            let shelf_move = fields.get("type").and_then(Value::as_str) == Some("move_shelf");
            for key in [
                "width",
                "height",
                "depth",
                "elevation",
                "x",
                "before",
                "after",
            ] {
                if matches!(key, "before" | "after") && !shelf_move {
                    continue;
                }
                let suffixed = format!("{key}_sixteenths");
                let (from, to) = if exporting {
                    (key, suffixed.as_str())
                } else {
                    (suffixed.as_str(), key)
                };
                if let Some(value) = fields.remove(from) {
                    if fields.insert(to.to_string(), value).is_some() {
                        return Err("Ambiguous geometry fields in bay file.".into());
                    }
                } else if !exporting && fields.contains_key(key) {
                    return Err("Bay geometry must use explicit sixteenths fields.".into());
                }
            }
            for value in fields.values_mut() {
                geometry_keys(value, exporting)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn committed_layout_round_trip_preserves_catalog_and_undo_then_allocates_fresh_ids() {
        use planogram_core::{CommandResult, Length, ProductId, ShelfId};
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        assert!(matches!(
            draft.add_placement(
                &version,
                &ProductId::new("jif_creamy_16"),
                &ShelfId::new("shelf_01"),
                0,
                "Add tray"
            ),
            CommandResult::Applied { .. }
        ));
        assert!(matches!(
            draft.move_shelf(
                &version,
                &ShelfId::new("shelf_06"),
                Length::inches(73),
                1,
                "Raise shelf"
            ),
            CommandResult::Applied { .. }
        ));
        let json = export(&draft, "Mixed bay").unwrap();
        assert!(json.contains("before_sixteenths"));
        assert!(json.contains("after_sixteenths"));
        let (_, mut reopened) = parse(&json).unwrap();
        assert_eq!(draft, reopened);
        let change = reopened.latest_undoable_change_set_id().unwrap().clone();
        assert!(matches!(
            reopened.undo_change_set(&version, &change, 2),
            CommandResult::Applied { revision: 3, .. }
        ));
        assert_eq!(reopened.fixture, planogram_core::default_fixture());
        assert!(matches!(
            reopened.add_placement(
                &version,
                &ProductId::new("jif_creamy_16"),
                &ShelfId::new("shelf_01"),
                3,
                "Continue editing"
            ),
            CommandResult::Applied { revision: 4, .. }
        ));
        assert_eq!(reopened.placements[1].id.0, "placement_0002");
        assert_eq!(
            parse(&export(&reopened, "Mixed bay").unwrap()).unwrap().1,
            reopened
        );
    }

    #[test]
    fn file_round_trip_and_errors() {
        let draft = DraftVersion::default();
        let json = export(&draft, "Peanut butter bay").unwrap();
        assert!(json.contains("width_sixteenths"));
        assert_eq!(parse(&json).unwrap(), ("Peanut butter bay".into(), draft));
        assert!(
            parse(&json.replace("\"format_version\": 1", "\"format_version\": 2"))
                .unwrap_err()
                .contains("Unsupported")
        );
        for invalid in ["{}", "null", "{", "[]"] {
            assert!(parse(invalid).is_err());
        }
        assert!(parse(&json.replace("width_sixteenths", "width")).is_err());
        assert!(
            parse(&json.replace("\"revision\": 0", "\"revision\": 0, \"unexpected\": true"))
                .is_err()
        );
        assert!(parse(&" ".repeat(MAX_FILE_BYTES + 1)).is_err());
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioFile {
    format: String,
    format_version: u32,
    name: String,
    document: Value,
}
pub fn export_document(
    document: &crate::document::EditorDocument,
    name: &str,
) -> Result<String, String> {
    if let crate::document::EditorDocument::Bay { draft } = document {
        return export(draft, name);
    }
    validate_name(name)?;
    document.validate()?;
    let mut value = serde_json::to_value(document).map_err(|e| e.to_string())?;
    geometry_keys(&mut value, true)?;
    let json = serde_json::to_string_pretty(&ScenarioFile {
        format: "planogrammy-bay".into(),
        format_version: 2,
        name: name.into(),
        document: value,
    })
    .map_err(|e| e.to_string())?;
    if json.len() > MAX_FILE_BYTES {
        return Err("This scenario exceeds the 8 MiB file limit.".into());
    }
    Ok(json)
}
pub fn parse_document(json: &str) -> Result<(String, crate::document::EditorDocument), String> {
    if json.len() > MAX_FILE_BYTES {
        return Err("Bay files must be 8 MiB or smaller.".into());
    }
    let value: Value =
        serde_json::from_str(json).map_err(|_| "This is not a valid Planogrammy bay file.")?;
    if value["format_version"] == 1 {
        let (name, draft) = parse(json)?;
        let document = crate::document::EditorDocument::Bay { draft };
        document.validate()?;
        return Ok((name, document));
    }
    if value["format_version"] != 2 || value["format"] != "planogrammy-bay" {
        return Err("Unsupported bay file format or version.".into());
    }
    let mut file: ScenarioFile =
        serde_json::from_value(value).map_err(|_| "Malformed scenario envelope.")?;
    validate_name(&file.name)?;
    geometry_keys(&mut file.document, false)?;
    let document: crate::document::EditorDocument =
        serde_json::from_value(file.document.clone()).map_err(|_| "Malformed scenario data.")?;
    if serde_json::to_value(&document).map_err(|e| e.to_string())? != file.document {
        return Err("Unrecognized or incomplete scenario data.".into());
    }
    document.validate()?;
    Ok((file.name, document))
}
