//! Saved-file compatibility. Opening a bay replays its history through the
//! current command engine, so any change to command semantics, default
//! geometry, the catalog or the cereal generator can make earlier downloads
//! unopenable. These committed files were written by earlier builds and must
//! keep opening unchanged.
//!
//! Never edit or regenerate an existing fixture to make this test pass: that
//! hides exactly the break it exists to catch. When a format or semantic change
//! is intentional, add an explicit migration and a new fixture instead.
//!
//! The tests check what the saved files record, not that today's commands
//! would produce the same history: default behavior (such as shelf spacing)
//! may change as long as earlier recorded histories still replay.
use crate::bay_file::{export_document, parse_document};
use crate::document::EditorDocument;
use planogram_core::{
    CommandResult, DraftVersion, FacingsRequest, Length, PlacementChange, PlacementId, ProductId,
    ShelfAllocationStrategy, ShelfDistribution, ShelfId, VersionStatus,
};

const FIXTURES: [(&str, &str); 2] = [
    (
        "standard-bay-v1.planogrammy.json",
        include_str!("../fixtures/standard-bay-v1.planogrammy.json"),
    ),
    (
        "cereal-scenario-v2.planogrammy.json",
        include_str!("../fixtures/cereal-scenario-v2.planogrammy.json"),
    ),
];

fn applied(result: CommandResult) {
    assert!(
        matches!(result, CommandResult::Applied { .. }),
        "fixture setup command failed: {result:?}"
    );
}

fn placement_on(draft: &DraftVersion, shelf_id: &str, index: usize) -> PlacementId {
    let mut placements = draft
        .placements
        .iter()
        .filter(|placement| placement.shelf_id.0 == shelf_id)
        .collect::<Vec<_>>();
    placements.sort_by_key(|placement| placement.x);
    placements[index].id.clone()
}

/// A standard bay whose history uses every recorded operation type, both
/// actors and a compensating undo.
fn standard_bay() -> EditorDocument {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let shelf = ShelfId::new;
    let product = ProductId::new;
    applied(draft.add_placement(
        &version,
        &product("jif_creamy_16"),
        &shelf("shelf_01"),
        0,
        "Add tray",
    ));
    applied(draft.add_placement(
        &version,
        &product("jif_crunchy_16"),
        &shelf("shelf_01"),
        1,
        "Add loose",
    ));
    applied(draft.add_placement_as(
        &version,
        &product("skippy_creamy_40"),
        &shelf("shelf_01"),
        2,
        "webmcp",
        "Agent add",
    ));
    let loose = placement_on(&draft, "shelf_01", 1);
    applied(draft.set_facings(
        &version,
        &loose,
        FacingsRequest {
            facings_x: Some(2),
            ..FacingsRequest::default()
        },
        3,
        "Widen",
    ));
    applied(draft.distribute_shelf(
        &version,
        &shelf("shelf_01"),
        ShelfDistribution::PackedLeft,
        4,
        "Pack left",
    ));
    applied(draft.apply_shelf_allocation_as(
        &version,
        &shelf("shelf_01"),
        ShelfAllocationStrategy::FillEvenly,
        5,
        "webmcp",
        "Fill evenly",
    ));
    applied(draft.apply_placement_changes(
        &version,
        &[PlacementChange::Add {
            placement_id: None,
            product_id: product("peter_pan_creamy_16"),
            shelf_id: shelf("shelf_02"),
            sequence: 0,
            resolved_x: None,
            facings_x: None,
            facings_y: None,
            facings_z: None,
        }],
        6,
        "Proposal add",
    ));
    let tray = placement_on(&draft, "shelf_02", 0);
    applied(draft.move_placement(
        &version,
        &tray,
        &shelf("shelf_02"),
        Length::from_sixteenths(8),
        7,
        "Nudge",
    ));
    applied(draft.move_shelf(
        &version,
        &shelf("shelf_06"),
        Length::inches(73),
        8,
        "Raise top shelf",
    ));
    applied(draft.remove_placement(&version, &tray, 9, "Remove"));
    let latest = draft.latest_undoable_change_set_id().unwrap().clone();
    applied(draft.undo_change_set(&version, &latest, 10));
    EditorDocument::Bay { draft }
}

/// A cereal scenario with an edited alternative, a duplicate and the locked
/// baseline selected, covering removal, re-add, facings, cross-bay moves and undo.
fn cereal_scenario() -> EditorDocument {
    let mut document = EditorDocument::cereal(20_260_930).unwrap();
    {
        let draft = document.draft_mut();
        let version = draft.id.clone();
        let removed = placement_on(draft, "bay_01_shelf_01", 0);
        let product = draft.placement(&removed).unwrap().product_id.clone();
        applied(draft.remove_placement(&version, &removed, 0, "Free space"));
        applied(draft.add_placement(
            &version,
            &product,
            &ShelfId::new("bay_01_shelf_01"),
            1,
            "Re-add",
        ));
        let incoming = placement_on(draft, "bay_02_shelf_01", 0);
        applied(draft.apply_placement_changes(
            &version,
            &[PlacementChange::Remove {
                placement_id: placement_on(draft, "bay_01_shelf_01", 0),
            }],
            2,
            "Remove lead SKU",
        ));
        applied(draft.apply_placement_changes(
            &version,
            &[PlacementChange::Move {
                placement_id: incoming,
                shelf_id: ShelfId::new("bay_01_shelf_01"),
                sequence: 0,
                resolved_x: None,
            }],
            3,
            "Cross-bay move",
        ));
        let latest = draft.latest_undoable_change_set_id().unwrap().clone();
        applied(draft.undo_change_set(&version, &latest, 4));
    }
    document.duplicate().unwrap();
    {
        let draft = document.draft_mut();
        let version = draft.id.clone();
        let revision = draft.revision;
        let target = placement_on(draft, "bay_03_shelf_02", 0);
        let facings_x = draft.placement(&target).unwrap().facings_x - 1;
        applied(draft.set_facings(
            &version,
            &target,
            FacingsRequest {
                facings_x: Some(facings_x),
                ..FacingsRequest::default()
            },
            revision,
            "Trim a facing",
        ));
    }
    document.select(-1).unwrap();
    document
}

/// Writes fixtures that are missing or empty; never overwrites committed ones.
/// Run with `cargo test -p planogram-wasm -- --ignored write_missing_fixtures`.
#[test]
#[ignore]
fn write_missing_fixtures() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    std::fs::create_dir_all(&directory).unwrap();
    for (file, document, name) in [
        (
            "standard-bay-v1.planogrammy.json",
            standard_bay(),
            "Standard bay fixture",
        ),
        (
            "cereal-scenario-v2.planogrammy.json",
            cereal_scenario(),
            "Cereal scenario fixture",
        ),
    ] {
        let path = directory.join(file);
        if std::fs::metadata(&path).map_or(true, |metadata| metadata.len() == 0) {
            std::fs::write(path, export_document(&document, name).unwrap() + "\n").unwrap();
        }
    }
}

#[test]
fn committed_files_from_earlier_builds_still_open() {
    for (file, json) in FIXTURES {
        let (_, document) =
            parse_document(json).unwrap_or_else(|error| panic!("{file} no longer opens: {error}"));
        assert!(document.draft().revision > 0 || matches!(document, EditorDocument::Cereal { .. }));
    }
}

#[test]
fn standard_bay_fixture_restores_its_recorded_state() {
    let (name, document) = parse_document(FIXTURES[0].1).unwrap();
    assert_eq!(name, "Standard bay fixture");
    let EditorDocument::Bay { draft } = document else {
        panic!("expected a version-1 bay");
    };
    assert_eq!(draft.revision, 11);
    assert_eq!(draft.change_sets.len(), 11);
    assert!(draft
        .change_sets
        .iter()
        .any(|change| change.actor == "webmcp"));
    assert!(draft.change_sets.last().unwrap().compensates.is_some());
    assert_eq!(draft.placements.len(), 4);
    assert_eq!(
        draft.shelf(&ShelfId::new("shelf_06")).unwrap().elevation,
        Length::inches(73)
    );
}

#[test]
fn cereal_fixture_restores_baseline_and_alternatives() {
    let (name, document) = parse_document(FIXTURES[1].1).unwrap();
    assert_eq!(name, "Cereal scenario fixture");
    let EditorDocument::Cereal {
        seed,
        baseline,
        alternatives,
        active,
        ..
    } = &document
    else {
        panic!("expected a version-2 cereal scenario");
    };
    assert_eq!(*seed, 20_260_930);
    assert_eq!(*active, None);
    assert_eq!(baseline.status, VersionStatus::Published);
    assert_eq!(baseline.fixture.sections.len(), 8);
    assert_eq!(alternatives.len(), 2);
    assert_eq!(alternatives[0].draft.revision, 5);
    assert_eq!(alternatives[1].draft.revision, 6);
    assert!(alternatives
        .iter()
        .all(|alternative| alternative.draft.status == VersionStatus::Draft));
}
