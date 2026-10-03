# Planogrammy Agent Guide

## Governing contract

- Treat `SPEC.md` as the governing product and architecture contract.
- Preserve later user-approved decisions recorded in this file when they narrow or revise the original slice requirements.
- This repository is a deterministic planogram editor, not a generic React canvas application.
- Do not add product catalogs, placements, persistence, authentication, WebMCP tools, publishing, collaboration, merchandising rules, or 3D behavior unless the task explicitly expands scope.

## Current vertical slice

The application displays one named `4' Standard Bay` fixture with:

- Width: 768 sixteenths (4 feet).
- Height: 1,344 sixteenths (7 feet).
- Fixture and base-deck depth: 352 sixteenths (22 inches).
- Adjustable-shelf depth: 256 sixteenths (16 inches).
- One fixed `BaseDeck` at elevation 0.
- Six `Adjustable` shelves at elevations 192, 384, 576, 768, 960, and 1,152.

All adjustable-shelf movement uses whole-inch increments: 16 internal units. Arrow keys, modified arrow keys, pointer snapping, and inspector commands must all respect this grid. The base deck is selectable for inspection but must never accept a valid move or drag operation.

Product x positions use 1/8-inch increments: 2 internal units. Distinct product placements on the same shelf must keep at least a 1/8-inch gap. Rust owns packing and shelf distribution; supported shelf layouts are packed left, centered, space between, and space evenly. Neighboring placements of the same SKU form a block that always keeps the minimum gap; every layout places its spacing only between blocks (and at the shelf ends for space evenly). The default layout is space-evenly blocks: direct and proposal adds, removals, and facing changes re-space the affected shelf in the same change set. A new placement joins an existing block of its SKU on the shelf, otherwise it starts a new block at the right end. Manual moves, explicit distributions, undo, and file replay keep exact positions; replay never re-applies the default layout. Distribution preserves stable left-to-right placement order, applies atomically as one revision/change set, and never lets React or WebMCP calculate final coordinates.

Shelf-facing allocation is also a semantic Rust-owned operation. The `fill_evenly` strategy resets loose products to one horizontal facing, repeatedly gives the next facing to the lowest-count loose placement that still fits using stable left-to-right order as the tie-breaker, preserves loaded-tray presets, and then distributes residual slack with the existing space-evenly resolver. Preview and apply must resolve from the same semantic shelf-and-strategy intent at the expected revision; the complete reflow commits as one change set and undo restores every exact prior position and facing count.

Manual facing changes are a semantic Rust-owned `set_facings` command shared by the inspector, the `+`/`-` keys on a selected placement, and the WebMCP `planogram.set_facings` tool. Omitted counts keep their current value. Rust then re-spaces the shelf in the default block layout. The `ChangeFacings` operation and every resulting move, including the resized placement's own, commit as one change set, bounds, clearance, depth, and tray-preset violations fail atomically, and one undo restores every exact prior position and count. Loaded trays reject any facings other than their preset.

The representative catalog contains 22 peanut-butter SKUs. Every product exposes its existing authoritative depth plus exact net weight in hundredths of an ounce, sales cents per store per week, unit milliunits per store per week, gross-margin basis points, and casepack quantity. Performance uses `period = "Trailing 13 weeks"` and the source label `Synthetic representative 13-week average; not retailer actuals`; do not present it as live retailer data or store these values as floating point.

Five products use a loaded tray configuration: `jif_creamy_16`, `skippy_creamy_16`, `peter_pan_creamy_16`, `smuckers_natural_16`, and `justins_classic_16`. The Rust fields are `outer_width`, `outer_height`, `outer_depth`, `front_lip_height`, `facings_x`, and `units_deep`; transport adapters add `_sixteenths` to the four `Length` values. These outer dimensions describe the loaded footprint. One loaded tray is one placement. Products without a tray configuration remain loose.

## Portable bay files

The first persistence slice is a local version-1 `.planogrammy.json` file, not a backend. It contains a document name and the complete committed `DraftVersion` including its catalog snapshot, exact geometry/facings/IDs, revisions, counters, and compensating change history. Pending proposals and camera/selection state are not committed data and must not be saved. Opening validates the entire snapshot and history through Rust before atomically replacing the engine state; failed or cancelled opens preserve current work. The browser tracks unsaved revisions, warns before replacement/navigation, and makes pending-proposal exclusion explicit. No login, cloud storage, PSA import, or new AI behavior is included.

## Repository boundaries

```text
apps/web/
  React/Vite shell, inspector, accessible companion, UI input adapters

crates/planogram-core/
  authoritative domain state, Length, entities, commands, validation,
  revisions, change sets, undo, render scenes, and scene patches

crates/planogram-render/
  Rust/wgpu WebGPU renderer, camera, hit testing, drag previews,
  snapping, selection, validation treatment, and patch application

crates/planogram-wasm/
  narrow wasm-bindgen boundary and transport/domain conversion
```

Dependency direction must point toward the domain. `planogram-core` must not depend on React, TypeScript, browser APIs, Wasm, `wgpu`, persistence, or WebMCP.

## Non-negotiable invariants

- `Length` is the authoritative geometry value type.
- Store geometry as integer sixteenths of an inch. Never store floating-point inches.
- Keep default fixture dimensions in named Rust domain data; do not repeat them in React or rendering code.
- Zustand may mirror revision, selection, command status, and inspector data only. It must not own an editable fixture model.
- Inspector, keyboard, and completed pointer drags must call the same semantic `moveShelf` adapter and Rust command.
- Every mutation includes `expectedRevision`.
- A successful move validates the complete proposal, changes Rust-owned state atomically, increments the revision once, records one change set, and returns a compact scene patch.
- A failed move must not change geometry, revision, or history.
- Product adds, moves, generic reflows, and distribution commands must all enforce the same 1/8-inch minimum inter-placement gap.
- Shelf distribution must resolve exact 1/8-inch positions in Rust, preserve stable placement order, and record all resulting placement moves in one atomic change set.
- Rust owns one tray-aware derived placement footprint used by add, move, preview, validation, distribution, render scenes, capacity, and proposal-impact views. React and WebMCP consume that Rust-derived view and must not multiply product or tray geometry independently.
- A tray's loaded footprint overrides loose product-dimension-times-facing geometry. Never multiply the loaded tray width by its preset facings again.
- Omitted add facings resolve in Rust to `1 × 1 × 1` for loose products or to the configured preset facings and units deep for tray products. Explicit tray-facing conflicts fail atomically without silent coercion.
- `casepack_quantity` counts sellable units in a vendor case; tray units are derived from preset facings times units deep. Keep these concepts distinct even when a representative case contains exactly one tray.
- Undo is a semantic command that records a compensating change set; it does not delete history.
- Shelves are ordered by elevation and then stable shelf ID. Renderer order must not decide domain order.
- Camera zoom, pan, viewport dimensions, and device-pixel ratio must never change authoritative geometry.
- Pointer movement updates renderer-owned preview state, not React state per pixel. Pointer release commits at most one semantic command.
- Do not replace the Rust/wgpu renderer with DOM shelf elements, SVG, Canvas 2D, or a second rendering engine.
- If WebGPU initialization fails, show the explicit unsupported-browser state.

## Imperial input boundary

The UI accepts forms such as:

- `24`
- `24"`
- `2'`
- `2' 6"`
- `30 1/2"`

A bare number means inches. Parsing belongs in TypeScript at the UI boundary. Reject precision finer than 1/16 inch without rounding. Rust validation additionally rejects shelf elevations that are not divisible by 16 internal units.

## Accessibility

Canvas interaction is not sufficient by itself. Keep an HTML companion that exposes:

- Fixture width and height.
- Base-deck details.
- All six adjustable shelves.
- Current selection, elevation, and depth.
- Selected product net weight, sales and units per store per week, gross-margin rate, casepack quantity, and synthetic source.
- Loaded tray dimensions, front-lip height, preset facings, and units deep when the selected product is trayed.
- Keyboard commands.
- Latest validation error.
- Current revision.

A keyboard user must be able to select an adjustable shelf, move it, edit its elevation, and undo without using the canvas.

## Editing practices

- Read `SPEC.md` and the affected Rust/TypeScript flow before changing ownership or commands.
- Trace behavior from the UI event through the shared adapter, Wasm boundary, Rust command, scene patch, renderer, and refreshed inspector state.
- Prefer domain types and exhaustive enums over loose primitives and duplicated conditionals.
- Keep transport fields explicit, including `_sixteenths` where the representation crosses a boundary.
- Keep non-geometric fixed-point units explicit in names too: `_cents`, `_milliunits`, `_basis_points`, and `_ounces_hundredths`.
- Preserve unrelated user changes and generated-artifact ignore rules.
- Do not commit generated Wasm bindings, `dist`, `target`, Playwright results, or TypeScript build info.

## Test conventions

- Rust domain tests live in `crates/planogram-core/src/tests.rs`. Use its helpers: `add` (checked setup add at the current revision), `add_change`, `expect_applied`, and `assert_rejected_unchanged` (asserts the validation code and that geometry, revision, and history are unchanged).
- Vitest data and canned Wasm results come from `apps/web/src/testFixtures.ts`. TypeScript tests check transport and routing only; they must not re-derive Rust geometry.
- Saved-file compatibility fixtures live in `crates/planogram-wasm/fixtures/` and are checked by `crates/planogram-wasm/src/golden.rs`. Opening replays history through the current engine, so a failure there means a change broke earlier downloads. Never edit or regenerate a committed fixture to make it pass; add an explicit migration and a new fixture instead.
- Browser tests use the helpers at the top of `apps/web/tests/browser/editor.spec.ts` (`openEditor`, `openEditorWithSiteTools`, `callSiteTool`, `expectRevision`, `skipUnlessRevision`). Scope text assertions to a panel when the same value can appear in the canvas outline, inspector, and companion.

## Commands

Run from the repository root unless noted:

```bash
npm install
npm run dev
npm run wasm
npm run typecheck
npm test
npm run build
npm --workspace apps/web run test:browser
npm --workspace apps/web run test:browser -- --headed   # real WebGPU; headless skips render-backed flows

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The Wasm build requires the `wasm32-unknown-unknown` Rust target and a `wasm-bindgen` CLI version compatible with the locked Rust crate version.

## Verification standard

Before declaring an editor change complete:

1. Run Rust formatting, Clippy, and native tests.
2. Build the actual Wasm artifact.
3. Run TypeScript checking, frontend tests, and the production build.
4. Run browser tests headed so render-backed flows execute with real WebGPU. Headless Chromium skips those flows; do not present a skip as interaction proof.
5. Open the app at `http://127.0.0.1:4173` in an in-app browser with real WebGPU and exercise the affected behavior. Test-only changes may rely on the headed browser run instead.
6. Inspect the visible fixture, companion state, revision changes, validation messages, focus behavior, and browser console.
7. Distinguish automated checks, live browser proof, and any WebGPU behavior that could not be verified.

For movement changes, specifically prove:

- Each adjustable shelf is selectable.
- Arrow and pointer movement land on the whole-inch grid.
- Fractional-inch inspector moves are rejected without revision changes.
- The base deck remains fixed.
- Undo restores the exact previous elevation.
- Zoom and pan do not alter domain values.

For catalog or tray changes, specifically prove:

- All 22 products expose depth, net weight, fixed-point performance, casepack, period, and source.
- Exactly the five configured products expose their loaded tray details.
- A tray add creates one placement with the configured facings and units deep.
- Tray fit, collision, distribution, capacity, and proposal impact use the loaded tray footprint.
- Explicit conflicting tray facings fail without geometry, revision, or history changes.
- Catalog, inspector, accessible companion, and WebMCP product results agree on the metrics and tray configuration.

For shelf layout changes, specifically prove same-SKU blocks stay at the minimum gap, block spacing is even on the 1/8-inch grid after adds, removals and facing changes, manual moves keep exact positions, and committed fixture files still open.

For manual facing changes, specifically prove the shelf re-spaces into the default block layout on the 1/8-inch grid, overflow, clearance, depth, and tray-preset violations change nothing, and one undo restores every exact prior position and facing count.

For shelf-facing allocation, specifically prove preview is non-mutating, apply recomputes and validates the semantic intent in Rust, loose facing counts are as even as shelf capacity permits, no additional whole facing fits, residual slack is distributed on the 1/8-inch grid, loaded trays retain their presets, and one undo restores every exact prior configuration.

## Approved synthetic cereal challenge

The next bounded slice explicitly permits a seeded, synthetic-only 100-SKU cereal catalog (five fictional brands × five cereal families × four pack sizes), an immutable eight-bay baseline, and up to six editable six-bay alternatives. Each bay is four feet wide with five adjustable cereal shelves at 8/23/38/53/68 inches and an empty fixed base deck. Fixture sections own horizontal bay origins; placement x remains shelf-local. Elevation collisions and clearance are scoped to a section. Existing Rust commands, validation, revisions and compensating undo remain authoritative.

Generator version 1 is immutable genesis data for history replay. Scenario files use format version 2 and include seed, assumptions, baseline, alternatives, active view, catalog snapshots and complete histories. Standard single-bay files remain version 1 and readable. Document revision guards replacement across alternative switches even when domain revision numbers happen to match.

Comparison uses Rust-derived stocked units and SKU-deduplicated demand. Capacity, distinct assortment, missing SKU IDs, weighted days cover, per-SKU cover and the sum of per-SKU demand/capacity ratios are simulated planning quantities. The latter is a sum of fractional SKU capacity turnovers per week, not category inventory turns, trips, cases, labor or financial forecasts. Always expose missing assortment beside this proxy. More facings never increase assumed demand. No real data integration or financial-outcome comparison is in scope.

## Approved sales allocation slice

Sales allocation offers revenue versus unit contribution, horizontal display space versus facing counts, and an occupied adjustable shelf or one bay. The semantic request includes minimum and maximum horizontal facings per existing loose placement (1–100). Preserve every represented SKU, placement ID, shelf assignment, stable order, vertical/depth facings, and fixed loaded-tray preset. Deduplicate contribution by SKU over the selected scope. Zero-contribution products retain minimum representation only; missing/inconsistent metadata, all-zero contribution, and infeasible constraints fail atomically.

Rust owns deterministic integer weighted apportionment, exact grid-aligned block distribution, physical validation, and per-SKU before/after share reporting. This is a bounded greedy allocator, not a global optimization or sales forecast. Preview is non-mutating, approval recomputes the semantic request with version/revision guards, and one undo restores every exact prior configuration. Browser proposals additionally guard the document revision and are never persisted. Existing file formats and the immutable cereal generator remain unchanged. See `SALES_ALLOCATION.md` for the user workflow, algorithm, and limits.

## Approved days-of-supply overlay

The canvas offers a display-only "Days of supply" color mode beside the default brand colors. Rust derives `SkuSupply` per placed SKU: stocked units summed across all of its placements, assumed weekly demand, `days_supply_millidays` (units × 7 ÷ weekly units) and a `SupplyBand` (under 3, under 7, under 14, 14 or more days, or no demand). The 7-day line matches the scenario's "stocked SKUs under 7 days" metric, and the scenario comparison uses the same arithmetic.

Every render scene and scene patch carries the complete `sku_supply` list, because changing one placement moves the days of supply of every placement of that SKU. The renderer tints placements by band and never moves geometry. React only toggles the mode and tallies Rust bands for the legend; the inspector, companion, label overlay and WebMCP `get_section` show the same Rust values. Toggling the overlay never changes geometry, revision or history. Days of supply is a simulated planning quantity from synthetic demand, not a forecast.

## Approved baseline comparison view

While a cereal alternative is active, "Compare with baseline" stacks the locked eight-bay baseline above the editable alternative in the same Rust/wgpu canvas, at one scale and sharing x = 0, so removed bays read as a marked empty region. The comparison is a display preference held by the Wasm engine: it survives alternative switches and file opens, disappears while the baseline itself is viewed, and never changes geometry, revision or history. The baseline is never hit-tested, selected or dragged; selection, drag previews, labels and proposals apply only to the editable fixture. The renderer reports screen frames for the HTML captions. The scenario panel shows Rust metrics as before → after cards; any delta shown there is display arithmetic on those metrics.
