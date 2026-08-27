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

Product x positions use 1/8-inch increments: 2 internal units. Distinct product placements on the same shelf must keep at least a 1/8-inch gap. Rust owns packing and shelf distribution; supported shelf layouts are packed left, centered, space between, and space evenly. Distribution preserves stable left-to-right placement order, applies atomically as one revision/change set, and never lets React or WebMCP calculate final coordinates.

The representative catalog contains 22 peanut-butter SKUs. Every product exposes its existing authoritative depth plus exact net weight in hundredths of an ounce, sales cents per store per week, unit milliunits per store per week, gross-margin basis points, and casepack quantity. Performance uses `period = "Trailing 13 weeks"` and the source label `Synthetic representative 13-week average; not retailer actuals`; do not present it as live retailer data or store these values as floating point.

Five products use a loaded tray configuration: `jif_creamy_16`, `skippy_creamy_16`, `peter_pan_creamy_16`, `smuckers_natural_16`, and `justins_classic_16`. The Rust fields are `outer_width`, `outer_height`, `outer_depth`, `front_lip_height`, `facings_x`, and `units_deep`; transport adapters add `_sixteenths` to the four `Length` values. These outer dimensions describe the loaded footprint. One loaded tray is one placement. Products without a tray configuration remain loose.

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
- Use `apply_patch` for source edits.
- Do not commit generated Wasm bindings, `dist`, `target`, Playwright results, or TypeScript build info.

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
4. Run browser tests. The headless command-flow test may skip when headless Chromium cannot execute render-backed WebGPU commands; do not present a skip as interaction proof.
5. Open the app at `http://127.0.0.1:4173` in the Codex in-app browser and exercise the affected behavior with real WebGPU.
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
