use super::*;

fn shelf(id: &str) -> ShelfId {
    ShelfId::new(id)
}

fn product(id: &str) -> ProductId {
    ProductId::new(id)
}

/// A proposal add with explicit `(x, y, z)` facings, or `None` so Rust resolves them.
fn add_change(
    product_id: &str,
    shelf_id: &str,
    sequence: u32,
    facings: Option<(u32, u32, u32)>,
) -> PlacementChange {
    PlacementChange::Add {
        placement_id: None,
        product_id: product(product_id),
        shelf_id: shelf(shelf_id),
        sequence,
        resolved_x: None,
        facings_x: facings.map(|facings| facings.0),
        facings_y: facings.map(|facings| facings.1),
        facings_z: facings.map(|facings| facings.2),
    }
}

struct Applied {
    revision: u64,
    change_set: ChangeSet,
    affected_ids: Vec<String>,
    scene_patch: Box<ScenePatch>,
}

fn expect_applied(result: CommandResult) -> Applied {
    match result {
        CommandResult::Applied {
            revision,
            change_set,
            affected_ids,
            scene_patch,
            ..
        } => Applied {
            revision,
            change_set,
            affected_ids,
            scene_patch,
        },
        result => panic!("expected an applied command, got {result:?}"),
    }
}

/// Setup helper: a direct add at the current revision that must apply.
fn add(draft: &mut DraftVersion, product_id: &str, shelf_id: &str) -> Applied {
    let version = draft.id.clone();
    let revision = draft.revision;
    expect_applied(draft.add_placement(
        &version,
        &product(product_id),
        &shelf(shelf_id),
        revision,
        "test setup",
    ))
}

fn has_issue(validation: &ValidationSummary, code: ValidationCode) -> bool {
    validation.issues.iter().any(|issue| issue.code == code)
}

/// Runs a command that must fail validation with `code` without changing geometry,
/// revision, or history.
fn assert_rejected_unchanged(
    draft: &mut DraftVersion,
    code: ValidationCode,
    command: impl FnOnce(&mut DraftVersion) -> CommandResult,
) {
    let before = draft.clone();
    let result = command(draft);
    assert!(
        matches!(
            &result,
            CommandResult::ValidationFailed { revision, validation }
                if *revision == before.revision && has_issue(validation, code)
        ),
        "expected {code:?} at revision {}, got {result:?}",
        before.revision
    );
    assert_eq!(*draft, before);
}

/// Placement views on one shelf, in stored placement order.
fn shelf_views(draft: &DraftVersion, shelf_id: &str) -> Vec<PlacementView> {
    draft
        .placement_views()
        .into_iter()
        .filter(|view| view.shelf_id == shelf(shelf_id))
        .collect()
}

/// Asserts the default shelf layout: stable left-to-right order, same-SKU
/// neighbors at the minimum gap, and blocks spaced evenly across the shelf.
fn assert_even_blocks(draft: &DraftVersion, shelf_id: &str) {
    let mut views = shelf_views(draft, shelf_id);
    views.sort_by_key(|view| view.x);
    let widths = views
        .iter()
        .map(|view| view.geometry.display_width)
        .collect::<Vec<_>>();
    let joined = same_sku_runs(views.iter().map(|view| &view.product_id));
    let expected = resolve_block_distribution(
        &widths,
        &joined,
        draft.shelf(&shelf(shelf_id)).unwrap().width,
        ShelfDistribution::SpaceEvenly,
    )
    .expect("the shelf fits its products");
    assert_eq!(
        views.iter().map(|view| view.x).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn default_layout_keeps_same_sku_blocks_tight_and_spaces_blocks_evenly() {
    let mut draft = DraftVersion::default();
    add(&mut draft, "jif_crunchy_16", "shelf_01");
    add(&mut draft, "skippy_chunk_16", "shelf_01");
    // A second facing unit of the same SKU joins its block instead of the end.
    let joined = add(&mut draft, "jif_crunchy_16", "shelf_01");
    assert_eq!(joined.revision, 3);
    assert_eq!(joined.change_set.operations.len(), 3);

    let mut views = shelf_views(&draft, "shelf_01");
    views.sort_by_key(|view| view.x);
    let layout = views
        .iter()
        .map(|view| (view.product_id.0.as_str(), view.x.sixteenths()))
        .collect::<Vec<_>>();
    assert_eq!(
        layout,
        vec![
            ("jif_crunchy_16", 196),
            ("jif_crunchy_16", 256),
            ("skippy_chunk_16", 512),
        ]
    );
    assert_even_blocks(&draft, "shelf_01");

    // Removing re-spaces the remaining blocks in the same change set, and one
    // undo restores every exact prior position.
    let before = draft.placements.clone();
    let version = draft.id.clone();
    let removed = expect_applied(draft.remove_placement(
        &version,
        &views[2].id,
        3,
        "Remove the Skippy block",
    ));
    assert_eq!(removed.change_set.operations.len(), 3);
    assert_even_blocks(&draft, "shelf_01");
    expect_applied(draft.undo_change_set(&version, &removed.change_set.id, 4));
    let by_id = |placements: &[Placement]| {
        let mut sorted = placements.to_vec();
        sorted.sort_by(|left, right| left.id.cmp(&right.id));
        sorted
    };
    assert_eq!(by_id(&draft.placements), by_id(&before));
}

#[test]
fn block_distribution_matches_per_item_spacing_when_every_sku_differs() {
    let widths = [57, 59, 58].map(Length::from_sixteenths);
    for distribution in [
        ShelfDistribution::PackedLeft,
        ShelfDistribution::Centered,
        ShelfDistribution::SpaceBetween,
        ShelfDistribution::SpaceEvenly,
    ] {
        let per_item = resolve_shelf_distribution(&widths, DEFAULT_FIXTURE_WIDTH, distribution);
        let blocks = resolve_block_distribution(
            &widths,
            &[false, false, false],
            DEFAULT_FIXTURE_WIDTH,
            distribution,
        );
        assert_eq!(per_item, blocks);
    }
    // One block of three spaces evenly as a unit: tight inside, centered.
    let block = resolve_block_distribution(
        &widths,
        &[false, true, true],
        DEFAULT_FIXTURE_WIDTH,
        ShelfDistribution::SpaceEvenly,
    )
    .unwrap();
    assert_eq!(
        block.iter().map(|x| x.sixteenths()).collect::<Vec<_>>(),
        vec![294, 354, 416]
    );
}

#[test]
fn default_fixture_is_exact_and_deterministic() {
    let fixture = default_fixture();
    assert_eq!(fixture.width.sixteenths(), 768);
    assert_eq!(fixture.height.sixteenths(), 1_344);
    let shelves = fixture.sections[0]
        .shelves
        .iter()
        .map(|shelf| (shelf.kind, shelf.elevation.sixteenths()))
        .collect::<Vec<_>>();
    assert_eq!(
        shelves,
        vec![
            (ShelfKind::BaseDeck, 0),
            (ShelfKind::Adjustable, 192),
            (ShelfKind::Adjustable, 384),
            (ShelfKind::Adjustable, 576),
            (ShelfKind::Adjustable, 768),
            (ShelfKind::Adjustable, 960),
            (ShelfKind::Adjustable, 1_152),
        ]
    );
}

#[test]
fn exact_length_conversions() {
    assert_eq!(Length::inches(1).sixteenths(), 16);
    assert_eq!(Length::inches(12).sixteenths(), 192);
    assert_eq!(Length::feet(1).sixteenths(), 192);
    assert_eq!(Length::from_sixteenths(200).sixteenths(), 200);
}

#[test]
fn whole_plan_validation_reports_the_current_revision_without_mutation() {
    let draft = DraftVersion::default();
    let before = draft.clone();

    let result = draft.validate_planogram();

    assert_eq!(result.revision, 0);
    assert!(result.valid);
    assert!(result.validation.issues.is_empty());
    assert_eq!(draft, before);
}

#[test]
fn whole_plan_validation_collects_structured_issues_across_the_draft() {
    let mut draft = DraftVersion::default();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    add(&mut draft, "jif_creamy_16", "shelf_01");

    draft.placements[0].product_id = product("missing_product");
    draft.placements[0].shelf_id = shelf("missing_shelf");
    draft.placements[1].x = Length::from_sixteenths(1);
    let shelf_01_elevation = draft.shelf(&shelf("shelf_01")).unwrap().elevation;
    draft.shelf_mut(&shelf("shelf_02")).unwrap().elevation = shelf_01_elevation;
    let before = draft.clone();

    let result = draft.validate_planogram();

    assert_eq!(result.revision, 2);
    assert!(!result.valid);
    for code in [
        ValidationCode::MissingProduct,
        ValidationCode::MissingShelf,
        ValidationCode::PlacementXIncrement,
        ValidationCode::DuplicateElevation,
    ] {
        assert!(has_issue(&result.validation, code), "missing {code:?}");
    }
    assert_eq!(draft, before);
}

#[test]
fn valid_move_increments_once_and_returns_patch() {
    let mut draft = DraftVersion::default();
    let applied = expect_applied(draft.move_shelf(
        &draft.id.clone(),
        &shelf("shelf_02"),
        Length::from_sixteenths(400),
        0,
        "Inspector edit",
    ));
    assert_eq!(applied.revision, 1);
    assert_eq!(applied.scene_patch.revision, 1);
    assert_eq!(draft.revision, 1);
    assert_eq!(draft.change_sets.len(), 1);
    assert_eq!(
        draft.shelf(&shelf("shelf_02")).unwrap().elevation,
        Length::from_sixteenths(400)
    );
}

#[test]
fn invalid_moves_are_atomic() {
    for (id, elevation) in [
        ("base_deck", 4),
        ("shelf_02", 192),
        ("shelf_02", 1_345),
        ("shelf_02", 0),
        ("shelf_02", 401),
    ] {
        let mut draft = DraftVersion::default();
        let result = draft.move_shelf(
            &draft.id.clone(),
            &shelf(id),
            Length::from_sixteenths(elevation),
            0,
            "Invalid",
        );
        assert!(matches!(result, CommandResult::ValidationFailed { .. }));
        assert_eq!(draft, DraftVersion::default());
    }
}

#[test]
fn shelf_move_validates_derived_vertical_facings_atomically() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let added = draft.apply_placement_changes_as(
        &version,
        &[add_change("jif_crunchy_16", "shelf_01", 0, Some((1, 2, 1)))],
        0,
        "webmcp",
        "Stack the display two high",
    );
    assert_eq!(expect_applied(added).revision, 1);

    assert_rejected_unchanged(&mut draft, ValidationCode::ProductTooTall, |draft| {
        draft.move_shelf(
            &version,
            &shelf("shelf_02"),
            Length::from_sixteenths(320),
            1,
            "Reduce clearance",
        )
    });
}

#[test]
fn stale_revision_is_atomic() {
    let mut draft = DraftVersion::default();
    let result = draft.move_shelf(
        &draft.id.clone(),
        &shelf("shelf_02"),
        Length::from_sixteenths(400),
        9,
        "Stale",
    );
    assert!(matches!(
        result,
        CommandResult::RevisionConflict {
            current_revision: 0,
            ..
        }
    ));
    assert_eq!(draft, DraftVersion::default());
}

#[test]
fn undo_walks_backward_through_active_change_sets_and_preserves_history() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let first_id = expect_applied(draft.move_shelf(
        &version,
        &shelf("shelf_02"),
        Length::from_sixteenths(400),
        0,
        "Move shelf 02",
    ))
    .change_set
    .id;
    let second_id = expect_applied(draft.move_shelf(
        &version,
        &shelf("shelf_03"),
        Length::from_sixteenths(384),
        1,
        "Move shelf 03 into the old opening",
    ))
    .change_set
    .id;
    let before = draft.clone();

    let rejected = draft.undo_change_set(&version, &first_id, 2);
    assert!(matches!(
        rejected,
        CommandResult::InvalidCommand { ref message }
            if message == "Only the latest active change set is eligible for undo."
    ));
    assert_eq!(draft, before);

    assert_eq!(
        expect_applied(draft.undo_change_set(&version, &second_id, 2)).revision,
        3
    );
    assert_eq!(
        draft.shelf(&shelf("shelf_02")).unwrap().elevation,
        Length::from_sixteenths(400)
    );
    assert_eq!(
        draft.shelf(&shelf("shelf_03")).unwrap().elevation,
        Length::from_sixteenths(576)
    );
    assert_eq!(draft.latest_undoable_change_set_id(), Some(&first_id));

    assert_eq!(
        expect_applied(draft.undo_change_set(&version, &first_id, 3)).revision,
        4
    );
    assert_eq!(
        draft.shelf(&shelf("shelf_02")).unwrap().elevation,
        Length::from_sixteenths(384)
    );
    assert_eq!(draft.change_sets.len(), 4);
    assert_eq!(draft.change_sets[2].compensates, Some(second_id));
    assert_eq!(draft.change_sets[3].compensates, Some(first_id));
    assert_eq!(draft.latest_undoable_change_set_id(), None);
}

#[test]
fn product_placement_is_evenly_spaced_revisioned_and_only_latest_active_change_is_undoable() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let first = add(&mut draft, "jif_creamy_16", "shelf_01");
    assert_eq!(first.revision, 1);
    // A lone 175-sixteenth tray centers: 593 of slack, 296 steps split 148/148.
    assert_eq!(
        first.scene_patch.placements[0].x,
        Length::from_sixteenths(296)
    );
    let second = add(&mut draft, "skippy_creamy_16", "shelf_01");
    assert_eq!(second.revision, 2);
    assert_eq!(second.scene_patch.placements.len(), 2);
    assert_even_blocks(&draft, "shelf_01");

    let before_non_latest_undo = draft.clone();
    let rejected = draft.undo_change_set(&version, &first.change_set.id, 2);
    assert!(matches!(rejected, CommandResult::InvalidCommand { .. }));
    assert_eq!(draft, before_non_latest_undo);

    let undo = expect_applied(draft.undo_change_set(&version, &second.change_set.id, 2));
    assert_eq!(undo.revision, 3);
    assert_eq!(draft.placements.len(), 1);
    assert_eq!(draft.placements[0].product_id, product("jif_creamy_16"));

    let blocked_move = draft.move_shelf(
        &version,
        &shelf("shelf_02"),
        Length::inches(15),
        3,
        "reduce clearance",
    );
    assert!(matches!(
        blocked_move,
        CommandResult::ValidationFailed { revision: 3, .. }
    ));
    assert_eq!(
        draft.shelf(&shelf("shelf_02")).unwrap().elevation,
        Length::inches(24)
    );
}

#[test]
fn direct_add_rejects_the_fixed_base_deck_for_human_and_webmcp() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    assert_rejected_unchanged(&mut draft, ValidationCode::PlacementOnFixedShelf, |draft| {
        draft.add_placement(
            &version,
            &product("jif_creamy_16"),
            &shelf("base_deck"),
            0,
            "catalog add",
        )
    });
    assert_rejected_unchanged(&mut draft, ValidationCode::PlacementOnFixedShelf, |draft| {
        draft.add_placement_as(
            &version,
            &product("jif_creamy_16"),
            &shelf("base_deck"),
            0,
            "webmcp",
            "site-tool add",
        )
    });
}

#[test]
fn webmcp_add_and_undo_are_explicitly_attributed() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let added = expect_applied(draft.add_placement_as(
        &version,
        &product("jif_creamy_16"),
        &shelf("shelf_01"),
        0,
        "webmcp",
        "Add Jif from the site tool",
    ));
    assert_eq!(added.change_set.actor, "webmcp");
    assert_eq!(added.change_set.reason, "Add Jif from the site tool");

    let undone =
        expect_applied(draft.undo_change_set_as(&version, &added.change_set.id, 1, "webmcp"));
    assert_eq!(undone.change_set.actor, "webmcp");
    assert_eq!(undone.change_set.compensates, Some(added.change_set.id));
}

#[test]
fn generic_placement_proposal_previews_applies_atomically_and_undoes_as_one_batch() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let changes = vec![
        add_change("jif_creamy_16", "shelf_01", 0, None),
        add_change("jif_creamy_40", "shelf_02", 0, Some((1, 1, 1))),
    ];

    let preview = draft.preview_placement_changes(&version, &changes, 0);
    assert!(
        matches!(preview, PreviewResult::Ready { revision: 0, ref operations, .. } if operations.len() == 2)
    );
    assert!(draft.placements.is_empty());
    assert_eq!(draft.revision, 0);
    assert!(draft.change_sets.is_empty());

    let applied = expect_applied(draft.apply_placement_changes_as(
        &version,
        &changes,
        0,
        "webmcp",
        "Group brands, then place smaller packages higher",
    ));
    assert_eq!(applied.revision, 1);
    assert_eq!(
        applied.affected_ids,
        vec!["placement_0001", "placement_0002"]
    );
    assert_eq!(applied.scene_patch.placements.len(), 2);
    assert_eq!(applied.change_set.actor, "webmcp");
    assert_eq!(applied.change_set.operations.len(), 2);
    assert_eq!(draft.placements.len(), 2);
    assert_eq!(draft.revision, 1);

    let stale_preview = draft.preview_placement_changes(&version, &changes, 0);
    assert!(matches!(
        stale_preview,
        PreviewResult::RevisionConflict {
            current_revision: 1,
            ..
        }
    ));

    let undone =
        expect_applied(draft.undo_change_set_as(&version, &applied.change_set.id, 1, "webmcp"));
    assert_eq!(undone.revision, 2);
    assert_eq!(undone.change_set.actor, "webmcp");
    assert_eq!(undone.change_set.compensates, Some(applied.change_set.id));
    assert_eq!(undone.change_set.operations.len(), 2);
    assert!(draft.placements.is_empty());
}

#[test]
fn invalid_generic_placement_proposal_is_atomic() {
    let draft = DraftVersion::default();
    let before = draft.clone();
    let preview = draft.preview_placement_changes(
        &draft.id,
        &[
            add_change("jif_crunchy_16", "shelf_01", 0, Some((1, 1, 1))),
            add_change("skippy_chunk_16", "shelf_01", 1, Some((100, 1, 1))),
        ],
        0,
    );
    assert!(matches!(
        preview,
        PreviewResult::ValidationFailed { ref validation, revision: 0 }
            if has_issue(validation, ValidationCode::PlacementOutOfBounds)
    ));
    assert_eq!(draft, before);
}

#[test]
fn generic_proposal_validates_implicit_reflow_before_preview_or_apply() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    for _ in 0..12 {
        add(&mut draft, "jif_crunchy_16", "shelf_01");
    }
    let before = draft.clone();
    let overflow = [add_change("jif_crunchy_16", "shelf_01", 0, Some((1, 1, 1)))];

    let preview = draft.preview_placement_changes(&version, &overflow, 12);
    assert!(matches!(
        preview,
        PreviewResult::ValidationFailed { revision: 12, ref validation }
            if has_issue(validation, ValidationCode::PlacementOutOfBounds)
    ));
    assert_eq!(draft, before);

    assert_rejected_unchanged(&mut draft, ValidationCode::PlacementOutOfBounds, |draft| {
        draft.apply_placement_changes_as(&version, &overflow, 12, "webmcp", "Overfill the shelf")
    });
}

#[test]
fn generic_sequence_resolves_even_physical_positions_without_model_coordinates() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let applied = draft.apply_placement_changes_as(
        &version,
        &[
            add_change("jif_crunchy_16", "shelf_01", 1, Some((1, 1, 1))),
            add_change("skippy_chunk_16", "shelf_01", 0, Some((1, 1, 1))),
        ],
        0,
        "webmcp",
        "Order the brand block",
    );
    assert_eq!(expect_applied(applied).revision, 1);
    let x_of = |product_id: &str| {
        draft
            .placements
            .iter()
            .find(|placement| placement.product_id == product(product_id))
            .unwrap()
            .x
    };
    assert!(x_of("skippy_chunk_16") < x_of("jif_crunchy_16"));
    assert_even_blocks(&draft, "shelf_01");
}

#[test]
fn generic_add_reflows_existing_items_and_batch_undo_restores_their_exact_positions() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let existing = draft.placements[0].clone();
    let applied = expect_applied(draft.apply_placement_changes_as(
        &version,
        &[add_change("skippy_creamy_16", "shelf_01", 0, None)],
        1,
        "webmcp",
        "Prepend the next brand block",
    ));
    assert_eq!(applied.change_set.operations.len(), 2);
    let skippy = draft
        .placements
        .iter()
        .find(|placement| placement.product_id == product("skippy_creamy_16"))
        .unwrap();
    assert!(skippy.x < draft.placement(&existing.id).unwrap().x);
    assert_even_blocks(&draft, "shelf_01");

    let undone =
        expect_applied(draft.undo_change_set_as(&version, &applied.change_set.id, 2, "webmcp"));
    assert_eq!(undone.revision, 3);
    assert_eq!(draft.placements, vec![existing]);
}

#[test]
fn placement_removal_is_revisioned_atomic_and_exactly_undoable() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let original = draft.placements[0].clone();

    let before = draft.clone();
    let stale = draft.remove_placement(&version, &original.id, 0, "stale remove");
    assert!(matches!(stale, CommandResult::RevisionConflict { .. }));
    assert_eq!(draft, before);

    let removed =
        expect_applied(draft.remove_placement(&version, &original.id, 1, "inspector remove"));
    assert_eq!(removed.revision, 2);
    assert_eq!(
        removed.scene_patch.removed_placement_ids,
        vec![original.id.clone()]
    );
    assert!(draft.placements.is_empty());

    let undone = expect_applied(draft.undo_change_set(&version, &removed.change_set.id, 2));
    assert_eq!(undone.revision, 3);
    assert_eq!(draft.placements, vec![original]);
}

#[test]
fn placement_move_uses_eighth_inch_grid_records_one_operation_and_undoes_exactly() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let original = draft.placements[0].clone();

    let moved = expect_applied(draft.move_placement(
        &version,
        &original.id,
        &shelf("shelf_02"),
        Length::from_sixteenths(2),
        1,
        "Inspector move",
    ));
    assert_eq!(moved.revision, 2);
    assert_eq!(moved.scene_patch.placements.len(), 1);
    assert_eq!(moved.scene_patch.placements[0].shelf_id, shelf("shelf_02"));
    assert_eq!(
        moved.scene_patch.placements[0].x,
        Length::from_sixteenths(2)
    );
    assert_eq!(moved.change_set.operations.len(), 1);
    match &moved.change_set.operations[0] {
        PlanogramOperation::MovePlacement(operation) => {
            assert_eq!(operation.placement_id, original.id);
            assert_eq!(operation.before.shelf_id, shelf("shelf_01"));
            assert_eq!(operation.after.shelf_id, shelf("shelf_02"));
            assert_eq!(operation.before.x, original.x);
            assert_eq!(operation.after.x, Length::from_sixteenths(2));
        }
        operation => panic!("unexpected operation: {operation:?}"),
    }
    assert_eq!(draft.revision, 2);
    assert_eq!(draft.change_sets.len(), 2);
    assert_eq!(draft.placements[0].shelf_id, shelf("shelf_02"));
    assert_eq!(draft.placements[0].x, Length::from_sixteenths(2));

    let undone = expect_applied(draft.undo_change_set(&version, &moved.change_set.id, 2));
    assert_eq!(undone.revision, 3);
    assert_eq!(draft.placements[0].shelf_id, original.shelf_id);
    assert_eq!(draft.placements[0].x, original.x);
    assert_eq!(draft.change_sets.len(), 3);
    assert_eq!(draft.change_sets[2].compensates, Some(moved.change_set.id));
}

#[test]
fn invalid_placement_moves_are_atomic_for_increment_and_shelf_bounds() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let placement_id = draft.placements[0].id.clone();

    for (target_shelf, target_x, code) in [
        ("shelf_01", 1, ValidationCode::PlacementXIncrement),
        ("shelf_01", 712, ValidationCode::PlacementOutOfBounds),
        ("base_deck", 0, ValidationCode::PlacementOnFixedShelf),
    ] {
        assert_rejected_unchanged(&mut draft, code, |draft| {
            draft.move_placement(
                &version,
                &placement_id,
                &shelf(target_shelf),
                Length::from_sixteenths(target_x),
                1,
                "Invalid move",
            )
        });
    }
}

#[test]
fn invalid_placement_moves_block_overlap_and_minimum_gap() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    add(&mut draft, "skippy_creamy_16", "shelf_01");
    let second_id = draft.placements[1].id.clone();
    // The 175-sixteenth tray ends at first.x + 175; +176 leaves a 1/16" gap.
    let first_x = draft.placements[0].x.sixteenths();

    for (target_x, code) in [
        (first_x, ValidationCode::PlacementOverlap),
        (first_x + 176, ValidationCode::PlacementGap),
    ] {
        assert_rejected_unchanged(&mut draft, code, |draft| {
            draft.move_placement(
                &version,
                &second_id,
                &shelf("shelf_01"),
                Length::from_sixteenths(target_x),
                2,
                "Invalid move",
            )
        });
    }
}

#[test]
fn invalid_placement_moves_block_shallow_shelves() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    draft.shelf_mut(&shelf("shelf_02")).unwrap().depth = Length::from_sixteenths(56);
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let placement_id = draft.placements[0].id.clone();

    assert_rejected_unchanged(&mut draft, ValidationCode::PlacementTooDeep, |draft| {
        draft.move_placement(
            &version,
            &placement_id,
            &shelf("shelf_02"),
            Length::ZERO,
            1,
            "depth",
        )
    });
}

#[test]
fn invalid_placement_moves_block_insufficient_clearance() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    draft.shelf_mut(&shelf("shelf_02")).unwrap().elevation = Length::from_sixteenths(500);
    draft.shelf_mut(&shelf("shelf_03")).unwrap().elevation = Length::from_sixteenths(550);
    add(&mut draft, "jif_creamy_40", "shelf_01");
    let placement_id = draft.placements[0].id.clone();

    assert_rejected_unchanged(&mut draft, ValidationCode::PlacementTooTall, |draft| {
        draft.move_placement(
            &version,
            &placement_id,
            &shelf("shelf_02"),
            Length::ZERO,
            1,
            "clearance",
        )
    });
}

#[test]
fn placement_move_rejects_stale_and_published_commands_without_mutation() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let placement_id = draft.placements[0].id.clone();
    let move_to_shelf_02 = |draft: &mut DraftVersion, expected_revision| {
        draft.move_placement(
            &version,
            &placement_id,
            &shelf("shelf_02"),
            Length::from_sixteenths(2),
            expected_revision,
            "rejected move",
        )
    };

    let before = draft.clone();
    let stale = move_to_shelf_02(&mut draft, 0);
    assert!(matches!(stale, CommandResult::RevisionConflict { .. }));
    assert_eq!(draft, before);

    draft.status = VersionStatus::Published;
    let before = draft.clone();
    let forbidden = move_to_shelf_02(&mut draft, 1);
    assert!(matches!(forbidden, CommandResult::Forbidden { .. }));
    assert_eq!(draft, before);
}

#[test]
fn placement_move_rejects_missing_version_placement_shelf_and_product_ids() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let not_found = |result: CommandResult| match result {
        CommandResult::NotFound { entity, .. } => entity,
        result => panic!("expected not found, got {result:?}"),
    };

    let missing = PlacementId::new("missing");
    assert_eq!(
        not_found(draft.move_placement(
            &version,
            &missing,
            &shelf("shelf_01"),
            Length::ZERO,
            0,
            "missing placement"
        )),
        "placement"
    );
    assert_eq!(
        not_found(draft.move_placement(
            &VersionId::new("wrong"),
            &missing,
            &shelf("shelf_01"),
            Length::ZERO,
            0,
            "wrong version",
        )),
        "version"
    );

    add(&mut draft, "jif_creamy_16", "shelf_01");
    let placement = draft.placements[0].clone();
    assert_eq!(
        not_found(draft.move_placement(
            &version,
            &placement.id,
            &shelf("missing"),
            Length::ZERO,
            1,
            "missing shelf"
        )),
        "shelf"
    );

    draft
        .products
        .retain(|product| product.id != placement.product_id);
    assert_eq!(
        not_found(draft.move_placement(
            &version,
            &placement.id,
            &shelf("shelf_02"),
            Length::ZERO,
            1,
            "missing product"
        )),
        "product"
    );
    assert_eq!(draft.revision, 1);
    assert_eq!(draft.change_sets.len(), 1);
}

#[test]
fn missing_placement_removal_does_not_mutate_state() {
    let mut draft = DraftVersion::default();
    let before = draft.clone();
    let result = draft.remove_placement(
        &draft.id.clone(),
        &PlacementId::new("missing"),
        0,
        "remove missing",
    );
    assert!(matches!(
        result,
        CommandResult::NotFound { ref entity, .. } if entity == "placement"
    ));
    assert_eq!(draft, before);
}

#[test]
fn undo_restores_legacy_odd_position_without_rounding() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    add(&mut draft, "jif_crunchy_16", "shelf_01");
    add(&mut draft, "skippy_chunk_16", "shelf_01");
    draft.placements[1].x = Length::from_sixteenths(59);
    let removed_id = draft.placements[1].id.clone();
    let removed = expect_applied(draft.remove_placement(&version, &removed_id, 2, "remove"));

    let undone = expect_applied(draft.undo_change_set(&version, &removed.change_set.id, 3));
    assert_eq!(undone.revision, 4);
    assert_eq!(
        draft.placement(&removed_id).unwrap().x,
        Length::from_sixteenths(59)
    );
}

#[test]
fn shelf_distribution_is_atomic_grid_aligned_balanced_and_undoable() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    for product_id in ["jif_creamy_16", "skippy_creamy_16", "peter_pan_creamy_16"] {
        add(&mut draft, product_id, "shelf_01");
    }
    // Adds already space blocks evenly; pack left first so the explicit
    // distribution has work to do.
    expect_applied(draft.distribute_shelf(
        &version,
        &shelf("shelf_01"),
        ShelfDistribution::PackedLeft,
        3,
        "inspector pack left",
    ));
    let before = draft.placements.clone();
    let applied = expect_applied(draft.distribute_shelf(
        &version,
        &shelf("shelf_01"),
        ShelfDistribution::SpaceEvenly,
        4,
        "inspector space evenly",
    ));
    assert_eq!(applied.revision, 5);
    assert_eq!(applied.change_set.operations.len(), 3);
    assert_eq!(applied.scene_patch.placements.len(), 3);

    let mut ordered = shelf_views(&draft, "shelf_01");
    ordered.sort_by_key(|view| view.x);
    assert_eq!(
        ordered.iter().map(|view| &view.id).collect::<Vec<_>>(),
        before
            .iter()
            .map(|placement| &placement.id)
            .collect::<Vec<_>>()
    );
    assert!(ordered.iter().all(|view| view.x.sixteenths() % 2 == 0));
    for pair in ordered.windows(2) {
        assert!(pair[1].x - (pair[0].x + pair[0].geometry.display_width) >= MIN_PLACEMENT_GAP);
    }
    let last = ordered.last().unwrap();
    let left_margin = ordered[0].x.sixteenths();
    let right_margin = (draft.shelf(&shelf("shelf_01")).unwrap().width
        - (last.x + last.geometry.display_width))
        .sixteenths();
    assert!((left_margin - right_margin).abs() <= 3);

    let undone = expect_applied(draft.undo_change_set(&version, &applied.change_set.id, 5));
    assert_eq!(undone.revision, 6);
    assert_eq!(draft.placements, before);
}

#[test]
fn fill_evenly_allocates_facings_in_rust_previews_applies_and_undoes_atomically() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let additions = ["jif_creamy_40", "jif_crunchy_40", "jif_natural_40"]
        .into_iter()
        .zip(0..)
        .map(|(product_id, sequence)| add_change(product_id, "shelf_01", sequence, None))
        .collect::<Vec<_>>();
    let added = draft.apply_placement_changes(&version, &additions, 0, "largest Jif assortment");
    assert_eq!(expect_applied(added).revision, 1);
    let before = draft.placements.clone();

    match draft.preview_shelf_allocation(
        &version,
        &shelf("shelf_01"),
        ShelfAllocationStrategy::FillEvenly,
        1,
    ) {
        PreviewResult::Ready {
            revision,
            operations,
            preview_scene,
            ref validation,
            ..
        } => {
            assert_eq!(revision, 1);
            assert!(validation.valid());
            assert_eq!(operations.len(), 3);
            assert!(operations
                .iter()
                .all(|operation| matches!(operation, PlanogramOperation::ReflowPlacement(_))));
            let proposed = preview_scene
                .placements
                .iter()
                .filter(|placement| placement.shelf_id == shelf("shelf_01"))
                .map(|placement| (placement.facings_x, placement.x.sixteenths()))
                .collect::<Vec<_>>();
            assert_eq!(proposed, vec![(4, 4), (4, 282), (3, 560)]);
        }
        result => panic!("unexpected preview: {result:?}"),
    }
    assert_eq!(draft.revision, 1);
    assert_eq!(draft.placements, before);

    let applied = expect_applied(draft.apply_shelf_allocation_as(
        &version,
        &shelf("shelf_01"),
        ShelfAllocationStrategy::FillEvenly,
        1,
        "webmcp",
        "Fill the bottom shelf evenly with facings",
    ));
    assert_eq!(applied.revision, 2);
    assert_eq!(applied.change_set.actor, "webmcp");
    assert_eq!(applied.change_set.operations.len(), 3);
    assert_eq!(applied.scene_patch.placements.len(), 3);

    let allocated = shelf_views(&draft, "shelf_01");
    assert_eq!(
        allocated
            .iter()
            .map(|view| view.facings_x)
            .collect::<Vec<_>>(),
        vec![4, 4, 3]
    );
    let occupied = allocated
        .iter()
        .map(|view| view.geometry.display_width.sixteenths())
        .sum::<i32>()
        + MIN_PLACEMENT_GAP.sixteenths() * 2;
    assert_eq!(occupied, 752);
    assert!(DEFAULT_FIXTURE_WIDTH.sixteenths() - occupied < 68);

    let undone = expect_applied(draft.undo_change_set(&version, &applied.change_set.id, 2));
    assert_eq!(undone.revision, 3);
    assert_eq!(draft.placements, before);
}

/// Requests only the named facing counts; omitted counts keep their current value.
fn facings(x: Option<u32>, y: Option<u32>, z: Option<u32>) -> FacingsRequest {
    FacingsRequest {
        facings_x: x,
        facings_y: y,
        facings_z: z,
    }
}

fn set_facings(
    draft: &mut DraftVersion,
    placement_id: &str,
    request: FacingsRequest,
) -> CommandResult {
    let version = draft.id.clone();
    let revision = draft.revision;
    draft.set_facings(
        &version,
        &PlacementId::new(placement_id),
        request,
        revision,
        "inspector set facings",
    )
}

#[test]
fn set_facings_respaces_the_shelf_in_one_change_set_and_undoes_exactly() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    for product_id in ["jif_crunchy_16", "skippy_chunk_16", "peter_pan_crunchy_16"] {
        add(&mut draft, product_id, "shelf_01");
    }
    let positions = |draft: &DraftVersion| {
        let mut views = shelf_views(draft, "shelf_01");
        views.sort_by_key(|view| view.x);
        views
            .iter()
            .map(|view| (view.id.0.clone(), view.x.sixteenths()))
            .collect::<Vec<_>>()
    };
    let at = |pairs: [(&str, i32); 3]| {
        pairs
            .iter()
            .map(|(id, x)| (id.to_string(), *x))
            .collect::<Vec<_>>()
    };
    // Widths 57, 59, 58: 588 of slack is 294 steps across four even slots.
    assert_eq!(
        positions(&draft),
        at([
            ("placement_0001", 148),
            ("placement_0002", 356),
            ("placement_0003", 564)
        ])
    );
    let before = draft.placements.clone();

    let widened = expect_applied(draft.set_facings_as(
        &version,
        &PlacementId::new("placement_0001"),
        facings(Some(3), None, None),
        3,
        "webmcp",
        "Give the lead item three facings",
    ));
    assert_eq!(widened.revision, 4);
    assert_eq!(widened.change_set.actor, "webmcp");
    assert_eq!(widened.scene_patch.placements.len(), 3);
    match widened.change_set.operations.as_slice() {
        [PlanogramOperation::ChangeFacings(change), moves @ ..] => {
            assert_eq!(change.placement_id, PlacementId::new("placement_0001"));
            assert_eq!((change.before.facings_x, change.after.facings_x), (1, 3));
            assert_eq!((change.after.facings_y, change.after.facings_z), (1, 1));
            // The resized placement itself shifts as part of the re-spacing.
            let shifts = moves
                .iter()
                .map(|operation| match operation {
                    PlanogramOperation::MovePlacement(movement) => (
                        movement.placement_id.0.as_str(),
                        movement.before.x.sixteenths(),
                        movement.after.x.sixteenths(),
                    ),
                    operation => panic!("unexpected operation: {operation:?}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                shifts,
                vec![
                    ("placement_0001", 148, 120),
                    ("placement_0002", 356, 412),
                    ("placement_0003", 564, 592),
                ]
            );
        }
        operations => panic!("unexpected operations: {operations:?}"),
    }
    assert_even_blocks(&draft, "shelf_01");

    let undone = expect_applied(draft.undo_change_set(&version, &widened.change_set.id, 4));
    assert_eq!(undone.revision, 5);
    assert_eq!(undone.change_set.compensates, Some(widened.change_set.id));
    assert_eq!(draft.placements, before);

    // Stacking vertically keeps the width, so the even layout is unchanged.
    let stacked = expect_applied(set_facings(
        &mut draft,
        "placement_0002",
        facings(None, Some(2), None),
    ));
    assert_eq!(stacked.change_set.operations.len(), 1);
    assert_eq!(
        positions(&draft),
        at([
            ("placement_0001", 148),
            ("placement_0002", 356),
            ("placement_0003", 564)
        ])
    );
    let stacked_view = draft
        .placement_view(&PlacementId::new("placement_0002"))
        .unwrap();
    assert_eq!(
        (stacked_view.facings_y, stacked_view.stocked_unit_count),
        (2, 2)
    );
}

#[test]
fn set_facings_rejections_leave_geometry_revision_and_history_unchanged() {
    let mut draft = DraftVersion::default();
    add(&mut draft, "jif_crunchy_16", "shelf_01");
    add(&mut draft, "jif_crunchy_16", "shelf_01");
    add(&mut draft, "jif_creamy_16", "shelf_02");

    // 3 × 78 exceeds the 12-inch clearance; 5 × 57 exceeds the 16-inch shelf depth.
    for (request, code) in [
        (
            facings(None, Some(3), None),
            ValidationCode::PlacementTooTall,
        ),
        (
            facings(None, None, Some(5)),
            ValidationCode::PlacementTooDeep,
        ),
        (
            facings(Some(0), None, None),
            ValidationCode::InvalidFacingCount,
        ),
        (
            facings(Some(101), None, None),
            ValidationCode::InvalidFacingCount,
        ),
        (
            facings(Some(13), None, None),
            ValidationCode::NoShelfCapacity,
        ),
    ] {
        assert_rejected_unchanged(&mut draft, code, |draft| {
            set_facings(draft, "placement_0001", request)
        });
    }
    for request in [
        facings(Some(4), None, None),
        facings(None, Some(2), None),
        facings(None, None, Some(3)),
    ] {
        assert_rejected_unchanged(&mut draft, ValidationCode::TrayFacingMismatch, |draft| {
            set_facings(draft, "placement_0003", request)
        });
    }

    let before = draft.clone();
    for request in [
        FacingsRequest::default(),
        facings(Some(1), Some(1), Some(1)),
    ] {
        assert!(matches!(
            set_facings(&mut draft, "placement_0001", request),
            CommandResult::InvalidCommand { .. }
        ));
    }
    assert!(matches!(
        set_facings(
            &mut draft,
            "placement_0003",
            facings(Some(3), Some(1), Some(4))
        ),
        CommandResult::InvalidCommand { .. }
    ));
    assert!(matches!(
        set_facings(&mut draft, "placement_9999", facings(Some(2), None, None)),
        CommandResult::NotFound { ref entity, .. } if entity == "placement"
    ));
    let version = draft.id.clone();
    assert!(matches!(
        draft.set_facings(
            &version,
            &PlacementId::new("placement_0001"),
            facings(Some(2), None, None),
            0,
            "stale",
        ),
        CommandResult::RevisionConflict {
            expected_revision: 0,
            current_revision: 3
        }
    ));
    assert_eq!(draft, before);
}

#[test]
fn distribution_modes_resolve_deterministically_on_the_eighth_inch_grid() {
    let widths = [
        Length::from_sixteenths(57),
        Length::from_sixteenths(60),
        Length::from_sixteenths(55),
    ];
    let shelf_width = Length::from_sixteenths(768);
    for distribution in [
        ShelfDistribution::PackedLeft,
        ShelfDistribution::Centered,
        ShelfDistribution::SpaceBetween,
        ShelfDistribution::SpaceEvenly,
    ] {
        let first = resolve_shelf_distribution(&widths, shelf_width, distribution).unwrap();
        let second = resolve_shelf_distribution(&widths, shelf_width, distribution).unwrap();
        assert_eq!(first, second);
        assert!(first.iter().all(|position| position.sixteenths() % 2 == 0));
        for index in 1..first.len() {
            assert!(first[index] - (first[index - 1] + widths[index - 1]) >= MIN_PLACEMENT_GAP);
        }
        assert!(*first.last().unwrap() + widths[widths.len() - 1] <= shelf_width);
    }
}

#[test]
fn duplicate_placement_targets_are_rejected_atomically() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let placement_id = draft.placements[0].id.clone();
    let before = draft.clone();
    let result = draft.preview_placement_changes(
        &version,
        &[
            PlacementChange::Move {
                placement_id: placement_id.clone(),
                shelf_id: shelf("shelf_02"),
                sequence: 0,
                resolved_x: None,
            },
            PlacementChange::Remove { placement_id },
        ],
        1,
    );
    assert!(matches!(
        result,
        PreviewResult::InvalidCommand { ref message }
            if message.contains("may only appear once")
    ));
    assert_eq!(draft, before);
}

#[test]
fn catalog_has_complete_fixed_point_metrics_and_exact_five_loaded_trays() {
    let draft = DraftVersion::default();
    let products = &draft.products;
    assert_eq!(products.len(), 22);
    assert!(draft.validate_planogram().valid);
    assert!(products.iter().all(|product| {
        product.dimensions.depth > Length::ZERO
            && product.net_weight_ounces_hundredths > 0
            && product.casepack_quantity > 0
            && product.performance.sales_per_store_per_week_cents > 0
            && product.performance.units_per_store_per_week_milliunits > 0
            && product.performance.gross_margin_basis_points > 0
            && product.performance.source == PERFORMANCE_SOURCE
            && product.performance.period == PERFORMANCE_PERIOD
    }));
    assert_eq!(
        products
            .iter()
            .map(|product| (
                product.id.0.as_str(),
                product.net_weight_ounces_hundredths,
                product.performance.sales_per_store_per_week_cents,
                product.performance.units_per_store_per_week_milliunits,
                product.performance.gross_margin_basis_points,
                product.casepack_quantity,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("jif_creamy_16", 1_600, 3_665, 10_500, 2_850, 12),
            ("jif_crunchy_16", 1_600, 2_024, 5_800, 2_875, 12),
            ("jif_natural_16", 1_600, 1_716, 4_300, 3_100, 12),
            ("jif_creamy_40", 4_000, 3_895, 5_200, 2_550, 6),
            ("jif_crunchy_40", 4_000, 1_947, 2_600, 2_575, 6),
            ("jif_natural_40", 4_000, 1_678, 2_100, 2_800, 6),
            ("skippy_creamy_16", 1_630, 2_928, 8_900, 2_900, 12),
            ("skippy_chunk_16", 1_630, 1_546, 4_700, 2_925, 12),
            ("skippy_natural_16", 1_500, 1_364, 3_600, 3_150, 12),
            ("skippy_creamy_40", 4_000, 3_076, 4_400, 2_600, 6),
            ("skippy_chunk_40", 4_000, 1_538, 2_200, 2_625, 6),
            ("skippy_natural_40", 4_000, 1_348, 1_800, 2_850, 6),
            ("peter_pan_creamy_16", 1_630, 1_914, 6_400, 2_750, 12),
            ("peter_pan_crunchy_16", 1_630, 927, 3_100, 2_775, 12),
            ("peter_pan_creamy_40", 4_000, 1_947, 3_000, 2_500, 6),
            ("peter_pan_crunchy_40", 4_000, 909, 1_400, 2_525, 6),
            ("smuckers_natural_16", 1_600, 1_572, 3_500, 3_300, 12),
            ("smuckers_chunky_16", 1_600, 808, 1_800, 3_325, 12),
            ("smuckers_natural_26", 2_600, 1_298, 2_000, 3_100, 6),
            ("smuckers_chunky_26", 2_600, 649, 1_000, 3_125, 6),
            ("justins_classic_16", 1_600, 1_118, 1_600, 3_600, 6),
            ("justins_classic_28", 2_800, 879, 800, 3_400, 6),
        ]
    );
    assert_eq!(
        products
            .iter()
            .filter_map(|product| {
                let tray = product.tray.as_ref()?;
                Some((
                    product.id.0.as_str(),
                    tray.facings_x,
                    tray.units_deep,
                    tray.outer_width.sixteenths(),
                    tray.outer_height.sixteenths(),
                    tray.outer_depth.sixteenths(),
                    tray.front_lip_height.sixteenths(),
                ))
            })
            .collect::<Vec<_>>(),
        vec![
            ("jif_creamy_16", 3, 4, 175, 80, 232, 20),
            ("skippy_creamy_16", 3, 4, 181, 78, 240, 20),
            ("peter_pan_creamy_16", 3, 4, 178, 78, 236, 20),
            ("smuckers_natural_16", 2, 3, 116, 84, 172, 20),
            ("justins_classic_16", 2, 3, 116, 86, 172, 20),
        ]
    );
}

#[test]
fn tray_direct_add_uses_one_loaded_footprint_everywhere() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let added = add(&mut draft, "jif_creamy_16", "shelf_01");
    assert_eq!(added.revision, 1);
    let node = &added.scene_patch.placements[0];
    assert_eq!(node.width, Length::from_sixteenths(175));
    assert_eq!(node.height, Length::from_sixteenths(80));
    assert_eq!(node.required_depth, Length::from_sixteenths(232));
    assert_eq!(node.stocking_mode, StockingMode::Tray);
    assert_eq!(node.stocked_unit_count, 12);
    assert_eq!((node.facings_x, node.facings_y, node.facings_z), (3, 1, 4));

    let placement_id = draft.placements[0].id.clone();
    let view = draft.placement_view(&placement_id).unwrap();
    assert_eq!(view.id, placement_id);
    assert_eq!(view.product_id, product("jif_creamy_16"));
    assert_eq!(view.stocking_mode, StockingMode::Tray);
    assert_eq!(view.stocked_unit_count, 12);
    assert_eq!(view.geometry.display_width, Length::from_sixteenths(175));

    draft.shelf_mut(&shelf("shelf_02")).unwrap().depth = Length::from_sixteenths(230);
    assert_rejected_unchanged(&mut draft, ValidationCode::PlacementTooDeep, |draft| {
        draft.move_placement(
            &version,
            &placement_id,
            &shelf("shelf_02"),
            Length::ZERO,
            1,
            "tray depth overflow",
        )
    });

    let undone = expect_applied(draft.undo_change_set(&version, &added.change_set.id, 1));
    assert_eq!(undone.revision, 2);
    assert!(draft.placements.is_empty());
}

#[test]
fn tray_proposals_resolve_omitted_facings_and_reject_explicit_conflicts_atomically() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let conflicting = [add_change("jif_creamy_16", "shelf_01", 0, Some((1, 1, 1)))];
    assert_rejected_unchanged(&mut draft, ValidationCode::TrayFacingMismatch, |draft| {
        draft.apply_placement_changes(&version, &conflicting, 0, "conflicting tray facings")
    });

    let omitted = [add_change("jif_creamy_16", "shelf_01", 0, None)];
    assert!(matches!(
        draft.preview_placement_changes(&version, &omitted, 0),
        PreviewResult::Ready { revision: 0, .. }
    ));
    let applied = draft.apply_placement_changes(&version, &omitted, 0, "stock tray");
    assert_eq!(expect_applied(applied).revision, 1);
    let placement = &draft.placements[0];
    assert_eq!(
        (
            placement.facings_x,
            placement.facings_y,
            placement.facings_z
        ),
        (3, 1, 4)
    );
}

#[test]
fn catalog_includes_near_eight_inch_family_size_variants_for_every_brand() {
    let products = default_products();
    let tall = products
        .iter()
        .filter(|product| product.dimensions.confidence == "concept")
        .collect::<Vec<_>>();
    assert_eq!(tall.len(), 11);
    assert!(tall.iter().all(|product| {
        (Length::from_sixteenths(120)..=Length::inches(8)).contains(&product.dimensions.height)
    }));
    for brand in ["Jif", "SKIPPY", "Peter Pan", "Smucker's", "Justin's"] {
        assert!(tall.iter().any(|product| product.brand == brand));
    }
}

#[test]
fn catalog_uses_one_color_per_brand() {
    let products = default_products();
    for brand in ["Jif", "SKIPPY", "Peter Pan", "Smucker's", "Justin's"] {
        let brand_colors = products
            .iter()
            .filter(|product| product.brand == brand)
            .map(|product| product.color)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(brand_colors.len(), 1, "{brand} should have one brand color");
    }
}

#[test]
fn snapshot_replays_all_command_types_and_compensating_history() {
    let mut draft = DraftVersion::default();
    let version = draft.id.clone();
    let target = shelf("shelf_01");
    draft.validate_snapshot().unwrap();
    let first = add(&mut draft, "jif_creamy_40", "shelf_01");
    let id = PlacementId::new(first.affected_ids[0].clone());
    add(&mut draft, "skippy_creamy_40", "shelf_01");
    draft.validate_snapshot().unwrap();
    expect_applied(draft.set_facings(
        &version,
        &id,
        FacingsRequest {
            facings_x: Some(2),
            ..Default::default()
        },
        draft.revision,
        "More facings",
    ));
    draft.validate_snapshot().unwrap();
    expect_applied(draft.distribute_shelf(
        &version,
        &target,
        ShelfDistribution::SpaceBetween,
        draft.revision,
        "Distribute",
    ));
    draft.validate_snapshot().unwrap();
    expect_applied(draft.apply_shelf_allocation_as(
        &version,
        &target,
        ShelfAllocationStrategy::FillEvenly,
        draft.revision,
        "webmcp",
        "Fill shelf",
    ));
    draft.validate_snapshot().unwrap();
    let last = draft.latest_undoable_change_set_id().unwrap().clone();
    expect_applied(draft.undo_change_set(&version, &last, draft.revision));
    draft.validate_snapshot().unwrap();
    expect_applied(draft.remove_placement(&version, &id, draft.revision, "Remove"));
    draft.validate_snapshot().unwrap();
    let last = draft.latest_undoable_change_set_id().unwrap().clone();
    expect_applied(draft.undo_change_set(&version, &last, draft.revision));
    draft.validate_snapshot().unwrap();
    expect_applied(draft.move_shelf(
        &version,
        &shelf("shelf_06"),
        Length::inches(73),
        draft.revision,
        "Move shelf",
    ));
    draft.validate_snapshot().unwrap();
    expect_applied(draft.apply_placement_changes(
        &version,
        &[
            add_change("jif_creamy_16", "shelf_02", 0, None),
            add_change("skippy_creamy_16", "shelf_02", 1, None),
        ],
        draft.revision,
        "Proposal",
    ));
    draft.validate_snapshot().unwrap();
    while let Some(last) = draft.latest_undoable_change_set_id().cloned() {
        expect_applied(draft.undo_change_set(&version, &last, draft.revision));
        draft.validate_snapshot().unwrap();
    }
    assert!(draft.placements.is_empty());
    assert_eq!(draft.fixture, default_fixture());
}

#[test]
fn snapshot_errors_name_the_first_change_set_that_fails_to_replay() {
    let mut draft = DraftVersion::default();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    add(&mut draft, "jif_creamy_16", "shelf_01");
    add(&mut draft, "jif_creamy_16", "shelf_01");

    let mut edited = draft.clone();
    edited.change_sets[1].base_revision = 99;
    let error = edited.validate_snapshot().unwrap_err();
    assert!(
        error.contains("change set change_0002 (2 of 3) replays to different operations"),
        "{error}"
    );

    let mut rejected = draft.clone();
    if let PlanogramOperation::AddPlacement(add) = &mut rejected.change_sets[0].operations[0] {
        add.placement.shelf_id = shelf("base_deck");
    }
    let error = rejected.validate_snapshot().unwrap_err();
    assert!(
        error.contains("change set change_0001 (1 of 3) is rejected by the current engine"),
        "{error}"
    );
}

#[test]
fn snapshot_rejects_corrupt_geometry_ids_counters_and_history_without_mutating() {
    let mut draft = DraftVersion::default();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let original = draft.clone();
    let corruptions: Vec<fn(&mut DraftVersion)> = vec![
        |draft| draft.next_placement = 1,
        |draft| draft.next_change_set = 1,
        |draft| draft.revision = u64::MAX,
        |draft| draft.placements[0].id = PlacementId::new("duplicate"),
        |draft| draft.placements.push(draft.placements[0].clone()),
        |draft| draft.placements[0].x = Length::from_sixteenths(i32::MAX),
        |draft| draft.products[0].dimensions.width = Length::from_sixteenths(i32::MAX),
        |draft| draft.fixture.sections[0].shelves[1].section_id = SectionId::new("missing"),
        |draft| draft.fixture.width = Length::ZERO,
        |draft| draft.change_sets[0].compensates = Some(ChangeSetId::new("missing")),
        |draft| draft.change_sets[0].operations.clear(),
        |draft| {
            draft.change_sets[0].operations = vec![PlanogramOperation::MoveShelf(MoveShelf {
                shelf_id: shelf("missing"),
                before: Length::ZERO,
                after: Length::from_sixteenths(i32::MAX),
            })]
        },
    ];
    for corrupt in corruptions {
        let mut candidate = original.clone();
        corrupt(&mut candidate);
        let before = candidate.clone();
        assert!(candidate.validate_snapshot().is_err());
        assert_eq!(candidate, before);
        assert_eq!(draft, original);
    }
}

#[test]
fn cereal_generation_is_reproducible_and_fits_all_supported_bays() {
    for seed in [0, 20_260_930, u32::MAX] {
        for bay_count in [6, 8] {
            let draft = cereal_draft(seed, bay_count).unwrap();
            assert_eq!(draft, cereal_draft(seed, bay_count).unwrap());
            assert_eq!(draft.products.len(), 100);
            assert_eq!(draft.placements.len(), 100);
            assert_eq!(draft.fixture.width, Length::feet(4 * bay_count as i32));
            assert_eq!(draft.fixture.height, Length::feet(7));
            assert_eq!(draft.fixture.sections.len(), bay_count as usize);
            assert_eq!(draft.revision, 0);
            assert!(draft.change_sets.is_empty());
            assert_eq!(draft.next_placement, 101);
            assert_eq!(draft.next_change_set, 1);
            assert!(draft.validate_planogram().valid);
            draft.validate_snapshot().unwrap();
            let mut ids = HashSet::new();
            for section in &draft.fixture.sections {
                assert_eq!(section.width, Length::feet(4));
                assert_eq!(section.shelves.len(), 6);
                for shelf in &section.shelves {
                    assert!(ids.insert(shelf.id.clone()));
                    assert_eq!(shelf.elevation.sixteenths() % 16, 0);
                    if shelf.kind == ShelfKind::BaseDeck {
                        assert_eq!(shelf.elevation, Length::ZERO);
                        assert!(!draft.placements.iter().any(|p| p.shelf_id == shelf.id));
                    }
                }
            }
            for view in draft.placement_views() {
                let shelf = draft.shelf(&view.shelf_id).unwrap();
                assert_eq!(view.x.sixteenths() % 2, 0);
                assert!(view.x + view.geometry.display_width <= shelf.width);
                assert!(view.geometry.required_depth <= shelf.depth);
                assert!(view.geometry.display_height <= draft.shelf_clearance(shelf));
                assert!(view.stocked_unit_count > 0);
            }
            assert_eq!(
                draft
                    .products
                    .iter()
                    .map(|p| &p.brand)
                    .collect::<HashSet<_>>()
                    .len(),
                5
            );
            assert!(draft
                .products
                .iter()
                .all(|p| p.performance.source.contains("synthetic")
                    && p.performance.units_per_store_per_week_milliunits > 0
                    && p.performance.units_per_store_per_week_milliunits < 100_000
                    && p.casepack_quantity > 0
                    && p.tray.is_none()));
        }
    }
    let original = cereal_draft(20_260_930, 6).unwrap();
    let changed_seed = cereal_draft(20_260_931, 6).unwrap();
    assert_ne!(original.products, changed_seed.products);
    for count in [0, 1, 4, 7, 9, 16, u32::MAX] {
        assert!(cereal_draft(20_260_930, count).is_err());
    }
}

#[test]
fn cereal_comparison_uses_explicit_fixed_point_units_and_full_assortment() {
    let baseline = cereal_draft(20_260_930, 8).unwrap();
    let target = cereal_draft(20_260_930, 6).unwrap();
    let comparison = compare_cereal(&baseline, &target);
    assert_eq!(comparison.products.len(), 100);
    assert_eq!(comparison.baseline.capacity_units, 1_475);
    assert_eq!(comparison.current.capacity_units, 1_095);
    assert_eq!(comparison.baseline.weekly_demand_milliunits, 1_677_501);
    assert_eq!(comparison.current.weekly_demand_milliunits, 1_677_501);
    assert_eq!(
        comparison.baseline.aggregate_days_supply_millidays,
        Some(6_154)
    );
    assert_eq!(
        comparison.current.aggregate_days_supply_millidays,
        Some(4_569)
    );
    assert_eq!(
        comparison.baseline.replenishment_turnovers_per_week_milli,
        129_656
    );
    assert_eq!(
        comparison.current.replenishment_turnovers_per_week_milli,
        174_249
    );
    assert_eq!(comparison.baseline.below_seven_days_sku_count, 67);
    assert_eq!(comparison.current.below_seven_days_sku_count, 81);
    for metrics in [&comparison.baseline, &comparison.current] {
        assert_eq!(metrics.distinct_sku_count, 100);
        assert_eq!(metrics.expected_sku_count, 100);
        assert!(metrics.unplaced_product_ids.is_empty());
        assert_eq!(metrics.validation_issue_count, 0);
    }
    assert!(!comparison.baseline.within_six_bay_limit);
    assert!(comparison.current.within_six_bay_limit);
    for row in comparison.products {
        assert_eq!(
            row.current_days_supply_millidays,
            Some(row.current_capacity_units * 7_000_000 / row.weekly_demand_milliunits)
        );
    }
    assert_eq!(baseline, cereal_draft(20_260_930, 8).unwrap());
    assert_eq!(target, cereal_draft(20_260_930, 6).unwrap());
}

#[test]
fn cereal_capacity_groups_duplicate_placements_without_counting_demand_twice() {
    let baseline = cereal_draft(20_260_930, 8).unwrap();
    let mut target = cereal_draft(20_260_930, 6).unwrap();
    let version = target.id.clone();
    let destination = shelf("bay_06_shelf_05");
    let removals = target
        .placements
        .iter()
        .filter(|p| p.shelf_id == destination)
        .map(|p| PlacementChange::Remove {
            placement_id: p.id.clone(),
        })
        .collect::<Vec<_>>();
    expect_applied(target.apply_placement_changes_as(&version, &removals, 0, "human", "Make room"));
    let before = compare_cereal(&baseline, &target);
    let existing_product = target.placements[0].product_id.clone();
    let added = add(&mut target, &existing_product.0, &destination.0);
    let new_id = PlacementId::new("placement_0101");
    assert!(added.affected_ids.contains(&new_id.0));
    let after = compare_cereal(&baseline, &target);
    assert_eq!(
        before.current.weekly_demand_milliunits,
        after.current.weekly_demand_milliunits
    );
    assert_eq!(
        before.current.stocked_weekly_demand_milliunits,
        after.current.stocked_weekly_demand_milliunits
    );
    assert_eq!(
        before.current.distinct_sku_count,
        after.current.distinct_sku_count
    );
    assert_eq!(
        after.current.capacity_units,
        before.current.capacity_units
            + u64::from(target.placement_view(&new_id).unwrap().stocked_unit_count)
    );
    target.validate_snapshot().unwrap();
}

#[test]
fn cereal_missing_skus_and_invalid_fit_stay_visible_without_reducing_demand() {
    let baseline = cereal_draft(20_260_930, 8).unwrap();
    let mut target = cereal_draft(20_260_930, 6).unwrap();
    let version = target.id.clone();
    let removed = target.placements[0].clone();
    expect_applied(target.remove_placement(&version, &removed.id, 0, "Remove one SKU"));
    let comparison = compare_cereal(&baseline, &target);
    assert_eq!(
        comparison.current.unplaced_product_ids,
        vec![removed.product_id.clone()]
    );
    assert_eq!(comparison.current.distinct_sku_count, 99);
    assert_eq!(comparison.current.expected_sku_count, 100);
    assert_eq!(
        comparison.current.weekly_demand_milliunits,
        comparison.baseline.weekly_demand_milliunits
    );
    let row = comparison
        .products
        .iter()
        .find(|row| row.product_id == removed.product_id)
        .unwrap();
    assert_eq!(row.current_capacity_units, 0);
    assert_eq!(row.current_days_supply_millidays, Some(0));
    target.validate_snapshot().unwrap();
    target.placements[0].x = Length::feet(4);
    assert!(
        compare_cereal(&baseline, &target)
            .current
            .validation_issue_count
            > 0
    );
    assert!(target.validate_snapshot().is_err());
}

#[test]
fn cereal_cross_bay_move_and_undo_preserve_exact_history_and_baseline() {
    let baseline = cereal_draft(20_260_930, 8).unwrap();
    let mut target = cereal_draft(20_260_930, 6).unwrap();
    let version = target.id.clone();
    let destination = shelf("bay_02_shelf_01");
    let removals = target
        .placements
        .iter()
        .filter(|p| p.shelf_id == destination)
        .map(|p| PlacementChange::Remove {
            placement_id: p.id.clone(),
        })
        .collect::<Vec<_>>();
    expect_applied(target.apply_placement_changes_as(
        &version,
        &removals,
        0,
        "human",
        "Clear target shelf",
    ));
    let original = target.clone();
    let placement = target.placements[0].clone();
    assert_ne!(
        target.shelf(&placement.shelf_id).unwrap().section_id,
        target.shelf(&destination).unwrap().section_id
    );
    let moved = expect_applied(target.move_placement(
        &version,
        &placement.id,
        &destination,
        Length::from_sixteenths(2),
        target.revision,
        "Move to neighboring bay",
    ));
    assert_eq!(
        target.placement(&placement.id).unwrap().shelf_id,
        destination
    );
    assert_eq!(
        target.placement(&placement.id).unwrap().x,
        Length::from_sixteenths(2)
    );
    assert_eq!(moved.change_set.operations.len(), 1);
    target.validate_snapshot().unwrap();
    let mut reopened = target.clone();
    let undone =
        expect_applied(reopened.undo_change_set(&version, &moved.change_set.id, reopened.revision));
    assert_eq!(reopened.placements, original.placements);
    assert_eq!(reopened.fixture, original.fixture);
    assert_eq!(undone.change_set.compensates, Some(moved.change_set.id));
    assert_eq!(reopened.revision, original.revision + 2);
    reopened.validate_snapshot().unwrap();
    assert_eq!(baseline, cereal_draft(20_260_930, 8).unwrap());
    let selected = shelf("bay_01_shelf_02");
    let shelf_move = expect_applied(reopened.move_shelf(
        &version,
        &selected,
        Length::inches(24),
        reopened.revision,
        "Move shelf inside its own bay",
    ));
    assert_eq!(
        reopened.shelf(&shelf("bay_02_shelf_02")).unwrap().elevation,
        Length::inches(23)
    );
    reopened.validate_snapshot().unwrap();
    expect_applied(reopened.undo_change_set(
        &version,
        &shelf_move.change_set.id,
        reopened.revision,
    ));
    reopened.validate_snapshot().unwrap();
}

#[test]
fn cereal_snapshot_rejects_changed_genesis_and_unsupported_generator_version() {
    let original = cereal_draft(20_260_930, 6).unwrap();
    let mut changed = original.clone();
    changed.products[0]
        .performance
        .units_per_store_per_week_milliunits += 1;
    assert!(changed.validate_snapshot().is_err());
    changed = original.clone();
    changed.fixture.sections[0].width = Length::inches(49);
    assert!(changed.validate_snapshot().is_err());
    changed = original.clone();
    changed.scenario_origin.as_mut().unwrap().generator_version = 2;
    assert!(changed.validate_snapshot().is_err());
    changed = original.clone();
    changed.scenario_origin.as_mut().unwrap().bay_count = 7;
    assert!(changed.validate_snapshot().is_err());
    assert_eq!(original, cereal_draft(20_260_930, 6).unwrap());
}

const SALES_TEST_PRODUCTS: [&str; 3] =
    ["jif_crunchy_16", "skippy_chunk_16", "peter_pan_crunchy_16"];

/// Small, explicitly synthetic examples make the chosen objective observable
/// without relying on the representative catalog's correlated sales figures.
fn sales_draft(inputs: &[(i32, u64, u64)]) -> DraftVersion {
    let mut draft = DraftVersion::default();
    for (id, &(width, revenue, units)) in SALES_TEST_PRODUCTS.iter().zip(inputs) {
        let catalog_product = draft
            .products
            .iter_mut()
            .find(|p| p.id == product(id))
            .unwrap();
        catalog_product.dimensions.width = Length::from_sixteenths(width);
        catalog_product.dimensions.height = Length::inches(2);
        catalog_product.dimensions.depth = Length::inches(2);
        catalog_product.performance.sales_per_store_per_week_cents = revenue;
        catalog_product
            .performance
            .units_per_store_per_week_milliunits = units;
        catalog_product.performance.source = "Synthetic allocation regression example".to_string();
        add(&mut draft, id, "shelf_01");
    }
    draft
}

fn sales_request(
    basis: SalesAllocationBasis,
    target: SalesAllocationTarget,
) -> SalesAllocationRequest {
    SalesAllocationRequest {
        scope: SalesAllocationScope::Shelf {
            shelf_id: shelf("shelf_01"),
        },
        basis,
        target,
        min_facings: 1,
        max_facings: 24,
    }
}

fn allocate_sales(draft: &mut DraftVersion, request: &SalesAllocationRequest) -> CommandResult {
    let version = draft.id.clone();
    let revision = draft.revision;
    draft.apply_sales_allocation_as(
        &version,
        request,
        revision,
        "test",
        "Synthetic sales allocation",
    )
}

#[test]
fn sales_allocation_revenue_and_units_follow_the_chosen_demand_basis() {
    let original = sales_draft(&[(40, 9_000, 1_000), (40, 1_000, 9_000)]);
    let mut by_revenue = original.clone();
    let mut by_units = original;
    expect_applied(allocate_sales(
        &mut by_revenue,
        &sales_request(
            SalesAllocationBasis::Revenue,
            SalesAllocationTarget::Facings,
        ),
    ));
    expect_applied(allocate_sales(
        &mut by_units,
        &sales_request(SalesAllocationBasis::Units, SalesAllocationTarget::Facings),
    ));
    assert!(by_revenue.placements[0].facings_x > by_revenue.placements[1].facings_x);
    assert!(by_units.placements[0].facings_x < by_units.placements[1].facings_x);
    // The stable left-to-right tie-break can give one residual facing to the
    // earlier SKU, so reversing demand need not produce a perfect mirror.
    assert!(
        by_revenue.placements[0]
            .facings_x
            .abs_diff(by_units.placements[1].facings_x)
            <= 1
    );
    assert!(
        by_revenue.placements[1]
            .facings_x
            .abs_diff(by_units.placements[0].facings_x)
            <= 1
    );
    assert!(by_revenue.validate_planogram().valid);
    assert!(by_units.validate_planogram().valid);
}

#[test]
fn sales_allocation_space_and_facings_are_distinct_objectives() {
    let original = sales_draft(&[(40, 1_000, 1_000), (80, 1_000, 1_000)]);
    let mut by_space = original.clone();
    let mut by_facings = original;
    expect_applied(allocate_sales(
        &mut by_space,
        &sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space),
    ));
    expect_applied(allocate_sales(
        &mut by_facings,
        &sales_request(
            SalesAllocationBasis::Revenue,
            SalesAllocationTarget::Facings,
        ),
    ));
    let space = shelf_views(&by_space, "shelf_01");
    let facings = shelf_views(&by_facings, "shelf_01");
    assert!(space[0].facings_x > space[1].facings_x);
    let space_imbalance = (space[0].geometry.display_width - space[1].geometry.display_width)
        .sixteenths()
        .abs();
    let facing_imbalance = (facings[0].geometry.display_width - facings[1].geometry.display_width)
        .sixteenths()
        .abs();
    assert!(
        space_imbalance < facing_imbalance,
        "Space allocation should bring equal-demand SKUs closer to equal width"
    );
    assert!(facings[0].facings_x.abs_diff(facings[1].facings_x) <= 1);
    assert_ne!(by_space.placements, by_facings.placements);
}

#[test]
fn sales_allocation_preview_is_deterministic_non_mutating_and_matches_apply_and_exact_undo() {
    let mut draft = sales_draft(&[(57, 1_200, 4_000), (59, 3_700, 2_000), (61, 2_600, 3_000)]);
    let id = draft.placements[0].id.clone();
    expect_applied(set_facings(
        &mut draft,
        &id.0,
        facings(None, Some(2), Some(3)),
    ));
    // A neighboring shelf's differently sourced data is outside this request.
    add(&mut draft, "jif_creamy_40", "shelf_02");
    let before = draft.clone();
    let version = draft.id.clone();
    let request = sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
    let preview = draft.preview_sales_allocation(&version, &request, draft.revision);
    for _ in 0..5 {
        assert_eq!(
            draft.preview_sales_allocation(&version, &request, draft.revision),
            preview
        );
    }
    let PreviewResult::Ready {
        operations,
        preview_scene,
        sales_allocation: Some(_),
        validation,
        ..
    } = preview
    else {
        panic!("expected a report and ready allocation preview");
    };
    assert!(validation.valid());
    assert_eq!(draft, before);
    let applied = expect_applied(allocate_sales(&mut draft, &request));
    assert_eq!(applied.revision, before.revision + 1);
    assert_eq!(draft.change_sets.len(), before.change_sets.len() + 1);
    assert_eq!(applied.change_set.operations, operations);
    assert_eq!(draft.render_scene().placements, preview_scene.placements);
    assert_eq!(
        draft.products, before.products,
        "Allocation must never change assumed demand"
    );
    for (after, prior) in draft.placements.iter().zip(&before.placements) {
        assert_eq!(
            (&after.id, &after.product_id, &after.shelf_id),
            (&prior.id, &prior.product_id, &prior.shelf_id)
        );
        assert_eq!(
            (after.facings_y, after.facings_z),
            (prior.facings_y, prior.facings_z)
        );
        if prior.shelf_id != shelf("shelf_01") {
            assert_eq!(after, prior);
        }
    }
    assert_even_blocks(&draft, "shelf_01");
    assert!(draft.validate_planogram().valid);
    let undone =
        expect_applied(draft.undo_change_set(&version, &applied.change_set.id, draft.revision));
    assert_eq!(undone.change_set.compensates, Some(applied.change_set.id));
    assert_eq!(draft.placements, before.placements);
    assert_eq!(draft.fixture, before.fixture);
    assert_eq!(draft.revision, before.revision + 2);
}

#[test]
fn sales_allocation_rejects_invalid_facing_bounds_and_infeasible_minima_atomically() {
    let mut draft = sales_draft(&[(100, 1_000, 1_000), (100, 1_000, 1_000)]);
    for (min_facings, max_facings) in [(0, 1), (1, 0), (3, 2), (1, 101), (101, 101)] {
        let mut request =
            sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
        request.min_facings = min_facings;
        request.max_facings = max_facings;
        assert_rejected_unchanged(&mut draft, ValidationCode::InvalidFacingCount, |draft| {
            allocate_sales(draft, &request)
        });
    }
    let mut request = sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
    request.min_facings = 4;
    assert_rejected_unchanged(&mut draft, ValidationCode::NoShelfCapacity, |draft| {
        allocate_sales(draft, &request)
    });
}

#[test]
fn sales_allocation_accounts_for_odd_width_grid_padding_when_minimum_looks_like_it_fits() {
    let mut draft = sales_draft(&[(84, 1_000, 1_000), (85, 1_000, 1_000), (85, 1_000, 1_000)]);
    draft.shelf_mut(&shelf("shelf_01")).unwrap().width = Length::from_sixteenths(766);
    assert!(draft.validate_planogram().valid);
    let mut request = sales_request(
        SalesAllocationBasis::Revenue,
        SalesAllocationTarget::Facings,
    );
    request.min_facings = 3;
    // 252 + 255 + 255 + two 2-unit gaps = 766, but the third item
    // must start on the even grid, so the arrangement actually needs 767.
    assert_rejected_unchanged(&mut draft, ValidationCode::NoShelfCapacity, |draft| {
        allocate_sales(draft, &request)
    });
}

#[test]
fn sales_allocation_with_odd_widths_is_grid_aligned_and_leaves_no_extra_facing_that_fits() {
    let mut draft = sales_draft(&[(57, 1_000, 1_000), (59, 2_000, 1_000), (61, 3_000, 1_000)]);
    expect_applied(allocate_sales(
        &mut draft,
        &sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space),
    ));
    assert!(draft.validate_planogram().valid);
    assert!(draft.placements.iter().all(|p| p.x.sixteenths() % 2 == 0));
    assert_even_blocks(&draft, "shelf_01");
    for placement in draft.placements.clone() {
        assert_rejected_unchanged(&mut draft, ValidationCode::NoShelfCapacity, |draft| {
            set_facings(
                draft,
                &placement.id.0,
                facings(Some(placement.facings_x + 1), None, None),
            )
        });
    }
}

#[test]
fn sales_allocation_zero_demand_stays_at_minimum_and_caps_leave_honest_slack() {
    let mut draft = sales_draft(&[(40, 0, 0), (40, 1_000, 1_000)]);
    let mut request = sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
    request.min_facings = 2;
    request.max_facings = 4;
    expect_applied(allocate_sales(&mut draft, &request));
    assert_eq!(draft.placements[0].facings_x, 2);
    assert_eq!(draft.placements[1].facings_x, 4);
    assert!(draft.validate_planogram().valid);
    let before = draft.clone();
    assert!(matches!(
        allocate_sales(&mut draft, &request),
        CommandResult::InvalidCommand { .. }
    ));
    assert_eq!(
        draft, before,
        "Repeating a resolved allocation must not create another revision"
    );
    for catalog_product in &mut draft.products {
        catalog_product.performance.sales_per_store_per_week_cents = 0;
    }
    assert_rejected_unchanged(
        &mut draft,
        ValidationCode::InvalidProductPerformance,
        |draft| allocate_sales(draft, &request),
    );
}

#[test]
fn sales_allocation_requires_coherent_sourced_performance_and_valid_catalog_dimensions() {
    let original = sales_draft(&[(40, 1_000, 1_000), (40, 1_000, 1_000)]);
    let request = sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
    for corrupt in [
        |p: &mut Product| p.performance.source.clear(),
        |p: &mut Product| p.performance.period.clear(),
        |p: &mut Product| p.performance.source = "A different synthetic source".into(),
        |p: &mut Product| p.performance.period = "A different synthetic period".into(),
    ] {
        let mut draft = original.clone();
        corrupt(
            draft
                .products
                .iter_mut()
                .find(|p| p.id == product(SALES_TEST_PRODUCTS[0]))
                .unwrap(),
        );
        let before = draft.clone();
        assert!(
            matches!(draft.preview_sales_allocation(&draft.id, &request, draft.revision), PreviewResult::ValidationFailed { ref validation, .. } if has_issue(validation, ValidationCode::InvalidProductPerformance))
        );
        assert_eq!(draft, before);
        assert_rejected_unchanged(
            &mut draft,
            ValidationCode::InvalidProductPerformance,
            |draft| allocate_sales(draft, &request),
        );
    }
    let mut draft = original;
    draft
        .products
        .iter_mut()
        .find(|p| p.id == product(SALES_TEST_PRODUCTS[0]))
        .unwrap()
        .dimensions
        .width = Length::ZERO;
    assert_rejected_unchanged(
        &mut draft,
        ValidationCode::InvalidProductDimensions,
        |draft| allocate_sales(draft, &request),
    );
}

#[test]
fn sales_allocation_preserves_loaded_tray_presets_even_outside_loose_facing_limits() {
    let mut draft = DraftVersion::default();
    add(&mut draft, "jif_creamy_16", "shelf_01");
    add(&mut draft, "jif_crunchy_16", "shelf_01");
    let tray_before = draft.placement_views()[0].clone();
    let mut request = sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
    request.max_facings = 2;
    expect_applied(allocate_sales(&mut draft, &request));
    let tray_after = draft
        .placement_views()
        .into_iter()
        .find(|p| p.id == tray_before.id)
        .unwrap();
    assert_eq!(
        (
            tray_after.facings_x,
            tray_after.facings_y,
            tray_after.facings_z
        ),
        (3, 1, 4)
    );
    assert_eq!(tray_after.geometry, tray_before.geometry);
    assert_eq!(
        tray_after.stocked_unit_count,
        tray_before.stocked_unit_count
    );
    assert!(draft
        .placements
        .iter()
        .filter(|p| p.product_id != tray_before.product_id)
        .all(|p| p.facings_x <= 2));
    assert!(draft.validate_planogram().valid);
}

#[test]
fn sales_allocation_rejects_empty_fixed_unknown_and_tray_only_scopes_without_changes() {
    let mut draft = DraftVersion::default();
    let mut request = sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
    let before = draft.clone();
    assert!(matches!(
        allocate_sales(&mut draft, &request),
        CommandResult::InvalidCommand { .. }
    ));
    assert_eq!(draft, before);
    request.scope = SalesAllocationScope::Shelf {
        shelf_id: shelf("base_deck"),
    };
    assert_rejected_unchanged(&mut draft, ValidationCode::PlacementOnFixedShelf, |draft| {
        allocate_sales(draft, &request)
    });
    for scope in [
        SalesAllocationScope::Shelf {
            shelf_id: shelf("missing"),
        },
        SalesAllocationScope::Bay {
            section_id: SectionId::new("missing"),
        },
    ] {
        request.scope = scope;
        assert!(matches!(
            allocate_sales(&mut draft, &request),
            CommandResult::NotFound { .. }
        ));
        assert_eq!(draft, before);
    }
    request.scope = SalesAllocationScope::Shelf {
        shelf_id: shelf("shelf_01"),
    };
    add(&mut draft, "jif_creamy_16", "shelf_01");
    let before = draft.clone();
    assert!(matches!(
        allocate_sales(&mut draft, &request),
        CommandResult::InvalidCommand { .. }
    ));
    assert_eq!(draft, before);
    draft.placements[0].product_id = product("missing");
    let before = draft.clone();
    assert!(matches!(
        allocate_sales(&mut draft, &request),
        CommandResult::NotFound { .. }
    ));
    assert_eq!(draft, before);
}

#[test]
fn sales_allocation_checks_version_revision_and_editability_before_any_mutation() {
    let mut draft = sales_draft(&[(40, 1_000, 1_000), (40, 2_000, 2_000)]);
    let request = sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
    let before = draft.clone();
    let version = draft.id.clone();
    assert!(matches!(
        draft.preview_sales_allocation(&version, &request, 0),
        PreviewResult::RevisionConflict {
            current_revision: 2,
            ..
        }
    ));
    assert!(matches!(
        draft.apply_sales_allocation_as(&version, &request, 0, "test", "stale"),
        CommandResult::RevisionConflict {
            current_revision: 2,
            ..
        }
    ));
    let missing = VersionId::new("missing");
    assert!(matches!(
        draft.preview_sales_allocation(&missing, &request, 2),
        PreviewResult::NotFound { .. }
    ));
    assert!(matches!(
        draft.apply_sales_allocation_as(&missing, &request, 2, "test", "wrong version"),
        CommandResult::NotFound { .. }
    ));
    assert_eq!(draft, before);
    for status in [VersionStatus::Published, VersionStatus::Archived] {
        draft.status = status;
        let before = draft.clone();
        assert!(matches!(
            draft.preview_sales_allocation(&version, &request, 2),
            PreviewResult::Forbidden { .. }
        ));
        assert!(matches!(
            allocate_sales(&mut draft, &request),
            CommandResult::Forbidden { .. }
        ));
        assert_eq!(draft, before);
    }
}

#[test]
fn sales_allocation_bay_scope_preserves_other_bays_assortment_and_replayable_undo() {
    let original = cereal_draft(20_260_930, 6).unwrap();
    for basis in [SalesAllocationBasis::Revenue, SalesAllocationBasis::Units] {
        for target in [SalesAllocationTarget::Space, SalesAllocationTarget::Facings] {
            let mut draft = original.clone();
            let request = SalesAllocationRequest {
                scope: SalesAllocationScope::Bay {
                    section_id: SectionId::new("bay_01"),
                },
                basis,
                target,
                min_facings: 1,
                max_facings: 24,
            };
            let applied = expect_applied(allocate_sales(&mut draft, &request));
            assert_eq!(draft.revision, 1);
            assert_eq!(draft.placements.len(), 100);
            assert_eq!(draft.products, original.products);
            assert_eq!(draft.fixture, original.fixture);
            for prior in &original.placements {
                let after = draft.placement(&prior.id).unwrap();
                assert_eq!(
                    (
                        &after.product_id,
                        &after.shelf_id,
                        after.facings_y,
                        after.facings_z
                    ),
                    (
                        &prior.product_id,
                        &prior.shelf_id,
                        prior.facings_y,
                        prior.facings_z
                    )
                );
                if draft.shelf(&prior.shelf_id).unwrap().section_id != SectionId::new("bay_01") {
                    assert_eq!(after, prior);
                }
            }
            assert!(draft.validate_planogram().valid);
            draft.validate_snapshot().unwrap();
            let version = draft.id.clone();
            expect_applied(draft.undo_change_set(&version, &applied.change_set.id, 1));
            assert_eq!(draft.placements, original.placements);
            draft.validate_snapshot().unwrap();
        }
    }
}

#[test]
fn sales_allocation_deduplicates_sku_demand_and_reports_actual_aggregate_geometry() {
    let mut draft = sales_draft(&[(40, 1_000, 1_000), (40, 1_000, 1_000)]);
    add(&mut draft, SALES_TEST_PRODUCTS[0], "shelf_01");
    let request = sales_request(SalesAllocationBasis::Units, SalesAllocationTarget::Facings);
    let PreviewResult::Ready {
        preview_scene,
        sales_allocation: Some(report),
        ..
    } = draft.preview_sales_allocation(&draft.id, &request, draft.revision)
    else {
        panic!("expected ready allocation with report");
    };
    assert_eq!(report.product_count, 2);
    assert_eq!(report.shelf_count, 1);
    assert_eq!(report.rows.len(), 2);
    assert_eq!(report.basis, SalesAllocationBasis::Units);
    assert_eq!(report.target, SalesAllocationTarget::Facings);
    assert_eq!(report.rows[0].before_facings, 2);
    assert_eq!(report.rows[1].before_facings, 1);
    for row in &report.rows {
        assert_eq!(
            row.contribution_basis_points, 5_000,
            "A duplicated placement must not double its SKU's demand"
        );
        let placements = preview_scene
            .placements
            .iter()
            .filter(|p| p.product_id == row.product_id)
            .collect::<Vec<_>>();
        assert_eq!(
            row.after_facings,
            placements.iter().map(|p| p.facings_x).sum::<u32>()
        );
        assert_eq!(
            row.after_space_sixteenths,
            placements
                .iter()
                .map(|p| i64::from(p.width.sixteenths()))
                .sum::<i64>()
        );
    }
    assert!(
        report.rows[0]
            .after_facings
            .abs_diff(report.rows[1].after_facings)
            <= 1
    );
    assert_eq!(
        report
            .rows
            .iter()
            .map(|r| r.contribution_basis_points)
            .sum::<u32>(),
        10_000
    );
    assert_eq!(
        report
            .rows
            .iter()
            .map(|r| r.before_share_basis_points)
            .sum::<u32>(),
        10_000
    );
    assert_eq!(
        report
            .rows
            .iter()
            .map(|r| r.after_share_basis_points)
            .sum::<u32>(),
        10_000
    );
    expect_applied(allocate_sales(&mut draft, &request));
    assert_even_blocks(&draft, "shelf_01");
    assert!(draft.validate_planogram().valid);
}

#[test]
fn sales_allocation_large_integer_demand_keeps_exact_deterministic_percentage_totals() {
    let draft = sales_draft(&[
        (40, u64::MAX, 1_000),
        (40, u64::MAX - 1, 1_000),
        (40, u64::MAX - 2, 1_000),
    ]);
    let request = sales_request(
        SalesAllocationBasis::Revenue,
        SalesAllocationTarget::Facings,
    );
    let first = draft.preview_sales_allocation(&draft.id, &request, draft.revision);
    assert_eq!(
        first,
        draft.preview_sales_allocation(&draft.id, &request, draft.revision)
    );
    let PreviewResult::Ready {
        sales_allocation: Some(report),
        ..
    } = first
    else {
        panic!("maximum integer demand should be safe to compare and aggregate");
    };
    assert_eq!(
        report
            .rows
            .iter()
            .map(|r| r.contribution_basis_points)
            .sum::<u32>(),
        10_000
    );
    assert_eq!(
        report
            .rows
            .iter()
            .map(|r| r.after_share_basis_points)
            .sum::<u32>(),
        10_000
    );
    assert!(report
        .rows
        .iter()
        .all(|r| (3_333..=3_334).contains(&r.contribution_basis_points)));
}

#[test]
fn sales_allocation_cereal_source_family_does_not_allow_unknown_or_malformed_sources() {
    let original = cereal_draft(20_260_930, 6).unwrap();
    let mut request = sales_request(SalesAllocationBasis::Revenue, SalesAllocationTarget::Space);
    request.scope = SalesAllocationScope::Bay {
        section_id: SectionId::new("bay_01"),
    };
    let first_id = original.placements[0].product_id.clone();
    for suffix in ["not-a-price", "0", "-1"] {
        let mut draft = original.clone();
        draft
            .products
            .iter_mut()
            .find(|p| p.id == first_id)
            .unwrap()
            .performance
            .source = format!(
            "{}; illustrative unit price {suffix} cents",
            cereal_assumptions().source
        );
        assert_rejected_unchanged(
            &mut draft,
            ValidationCode::InvalidProductPerformance,
            |draft| allocate_sales(draft, &request),
        );
    }
    // Only the immutable seeded cereal scenario defines that special source
    // convention. A standard draft cannot use it to combine unequal sources.
    let mut ordinary = sales_draft(&[(40, 1_000, 1_000), (40, 1_000, 1_000)]);
    for (id, price) in SALES_TEST_PRODUCTS.iter().zip([100, 200]) {
        ordinary
            .products
            .iter_mut()
            .find(|p| p.id == product(id))
            .unwrap()
            .performance
            .source = format!(
            "{}; illustrative unit price {price} cents",
            cereal_assumptions().source
        );
    }
    request.scope = SalesAllocationScope::Shelf {
        shelf_id: shelf("shelf_01"),
    };
    assert_rejected_unchanged(
        &mut ordinary,
        ValidationCode::InvalidProductPerformance,
        |draft| allocate_sales(draft, &request),
    );
}

#[test]
fn sales_allocation_infeasible_bay_minimum_changes_no_shelf_or_history() {
    let mut draft = cereal_draft(20_260_930, 6).unwrap();
    let request = SalesAllocationRequest {
        scope: SalesAllocationScope::Bay {
            section_id: SectionId::new("bay_01"),
        },
        basis: SalesAllocationBasis::Units,
        target: SalesAllocationTarget::Facings,
        min_facings: 100,
        max_facings: 100,
    };
    let before = draft.clone();
    assert!(
        matches!(draft.preview_sales_allocation(&draft.id, &request, 0), PreviewResult::ValidationFailed { ref validation, .. } if has_issue(validation, ValidationCode::NoShelfCapacity))
    );
    assert_eq!(draft, before);
    assert_rejected_unchanged(&mut draft, ValidationCode::NoShelfCapacity, |draft| {
        allocate_sales(draft, &request)
    });
}
