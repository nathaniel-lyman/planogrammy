# Planogrammy — Product and Technical Specification

**Status:** Draft 0.2
**Product:** Web-based planogram creation and editing application
**Primary integration:** OpenAI WebMCP site tools
**Canonical units:** Integer sixteenths of an inch

## 1. Product Summary

Planogrammy is a visual editor for creating, validating, versioning, and comparing retail planograms from a product catalog containing physical item dimensions.

The editor supports two equivalent control paths:

- Human interaction through drag, drop, keyboard, forms, and menus.
- AI-assisted interaction through semantic WebMCP tools.

Both paths must issue commands through the same planogram command bus. Neither the UI nor an AI agent may mutate canvas state directly.

The system assigns responsibilities as follows:

- The AI expresses merchandising intent.
- The domain engine calculates physical layout and enforces constraints.
- The renderer visualizes authoritative domain state.
- The user reviews, compares, accepts, or undoes changes.

## 2. Product Goals

1. Create and edit planograms against real product and fixture dimensions.
2. Make every edit deterministic, validated, reversible, and attributable.
3. Let ChatGPT inspect and edit the planogram currently open in the browser through WebMCP.
4. Prevent agents from manipulating pixels, guessing coordinates, or bypassing physical-fit rules.
5. Preserve published work by making alternatives and AI proposals as explicit versions or change sets.
6. Support progressively richer merchandising rules without coupling them to the renderer or AI integration.

## 3. Non-Goals for the First Release

- A 3D editing surface.
- A general-purpose optimization engine.
- A server-side MCP service for unattended or background work.
- GPU-based picking or compute-heavy layout optimization.
- Direct AI access to database CRUD operations.
- Silent AI mutation of a published planogram.

## 4. Core Design Principles

### 4.1 One command path

Mouse, keyboard, forms, imports, WebMCP, and future optimization jobs must use the same semantic command layer.

```text
Human UI ────────┐
Import pipeline ─┼──► PlanogramCommandBus ──► Domain engine ──► State
WebMCP tools ────┘                                  │
                                                   ├──► Validation
                                                   ├──► Change history
                                                   └──► Render scene
```

The UI must not directly mutate React state as the source of truth. A drag operation should commit a command such as:

```ts
movePlacement({
  placementId: "plc_19382",
  targetShelfId: "shelf_03",
  xSixteenths: 268,
  expectedRevision: 82,
});
```

### 4.2 Physical units, not screen units

All authoritative positions and dimensions use integer sixteenths of an inch. One inch equals 16 internal units. One foot equals 192 internal units. A 4-foot fixture is therefore exactly 768 internal units wide.

The domain engine owns a `Length` value type that stores the integer unit count. Geometry code must use `Length` instead of floating-point inches. Persistence and transport fields use the `_sixteenths` suffix so that callers know the representation.

User-facing controls display feet, inches, and common fractions. For example, the editor displays 768 internal units as `4'` and 200 internal units as `12 1/2"`. Input adapters may accept values such as `24`, `24"`, `2'`, `2' 6"`, and `30 1/2"`. A bare number represents inches. Input adapters must normalize accepted input before issuing a command.

The first release supports precision to 1/16 inch. An interactive UI or WebMCP input adapter must return a structured precision error when a value cannot be represented exactly. It must not round silently.

A catalog import may contain a finer measurement. The import adapter must apply the configured import policy and report every lossy conversion. The product team must choose that policy before catalog import work begins.

Pixels, zoom, device-pixel ratio, and viewport transforms are renderer concerns only.

```text
integer sixteenths of an inch
          ↓
camera-relative world coordinates
          ↓
viewport transform
          ↓
screen pixels
```

WebMCP tools must never accept raw screen coordinates. A tool that accepts an explicit physical offset must use integer sixteenths of an inch and name that representation in its schema.

### 4.3 Deterministic geometry

An LLM must not calculate final placement coordinates or determine whether a product fits. It may express intent such as adjacency, sequence, brand blocking, or minimum facings. The engine resolves that intent into valid placements.

### 4.4 Version-safe editing

Published planograms are immutable. Drafts, proposals, and store-specific variants branch from an explicit base version.

Every mutating command must include an `expectedRevision`. A stale command must fail without applying partial changes.

### 4.5 Reviewable AI changes

AI edits must produce a visible change set containing:

- Actor and reason.
- Base and resulting revision.
- Atomic operations.
- Physical-space and capacity impact.
- Validation result.
- Undo and comparison actions.

## 5. System Architecture

### 5.1 Baseline stack

- **Application shell:** Next.js, React, and TypeScript.
- **Domain engine:** Rust compiled to WebAssembly.
- **2D renderer:** Rust and `wgpu`, targeting WebGPU in the browser.
- **Client state adapter:** Zustand.
- **Persistence:** PostgreSQL behind application APIs.
- **AI integration:** Thin WebMCP adapter registered by the live page.

The first release uses a Rust-owned `wgpu` 2D front-elevation canvas. The renderer is part of the initial architecture, not a later replacement. Domain state, render-scene generation, GPU resource management, and interaction-critical canvas behavior remain on the Rust side of a narrow Wasm boundary.

### 5.2 Ownership boundaries

Rust owns correctness-sensitive and deterministic behavior:

- Product and fixture geometry.
- Catalog logistics, fixed-point performance values, and tray configuration validation.
- Authoritative planogram state.
- Commands and command results.
- Revisions, change sets, and undo/redo.
- Collision, overflow, depth, and clearance checks.
- Spatial indexing, snapping, and hit testing.
- Reflow and merchandising-constraint evaluation.
- Version comparison.
- Loaded placement footprints and the derived placement views consumed by the UI and WebMCP.
- Render-scene generation.

React and TypeScript own browser and application concerns:

- Catalog, inspector, property panels, menus, and dialogs.
- WebMCP registration and tool-result formatting.
- Pointer and keyboard command adapters.
- Accessible representations of canvas state and selection.
- Authentication, routing, API requests, and persistence.
- Import, export, clipboard, and other browser integrations.

Rust and `wgpu` own:

- WebGPU surface and canvas drawing.
- Viewport zoom and pan.
- Render pipelines, instance buffers, texture atlases, and GPU resources.
- Selection, drag-preview, snap-guide, and validation overlays.
- Semantic zoom and render-scene patch application.

### 5.3 Repository shape

```text
/apps/web
  React application shell
  WebMCP tool adapter
  persistence and API clients
  accessible canvas companion UI

/crates/planogram-core
  entities and identifiers
  commands and command results
  geometry and spatial index
  constraints and validation
  change sets and version comparison

/crates/planogram-wasm
  wasm-bindgen exports
  TypeScript-facing command adapter
  browser integration types

/crates/planogram-render
  wgpu initialization and browser surface
  render graph and pipelines
  instance-buffer management
  texture atlases
  camera and viewport transforms
  selection and validation overlays
```

`planogram-core` must not depend on WebMCP, browser APIs, Wasm, or `wgpu`. `planogram-render` consumes a render-oriented view of the domain and must never become the source of truth.

## 6. Domain Model

The planogram must be stored as normalized entities, not as one opaque canvas JSON document.

### 6.1 Product catalog

#### Product

- `id`
- `gtin` or `upc`
- `description`
- `brand`
- `category`
- `image_url`
- `net_weight_ounces_hundredths`
- `casepack_quantity`
- `performance`
- Optional `tray`
- Merchandising attributes
- Assortment eligibility

#### ProductDimensions

- `product_id`
- `width_sixteenths`
- `height_sixteenths`
- `depth_sixteenths`
- `source`
- `confidence`
- `measured_at`

`depth_sixteenths` is already part of the authoritative product geometry. Catalog rows, product details, placement inspection, the accessible companion, and WebMCP product results must expose it; no second product-depth field may be introduced.

`net_weight_ounces_hundredths` is the declared net contents in hundredths of an ounce. It is not gross shipping weight and must not be used for shelf-load validation. `casepack_quantity` is the positive number of sellable consumer units in one vendor case. Both are exact integers.

#### ProductPerformance

- `sales_per_store_per_week_cents`
- `units_per_store_per_week_milliunits`
- `gross_margin_basis_points`
- `period`
- `source`

The current 22-SKU representative catalog uses `period = "Trailing 13 weeks"` and the source label `Synthetic representative 13-week average; not retailer actuals`. Sales are integer cents, units are integer thousandths of a unit, and gross margin is an integer rate in basis points. Floating-point catalog values are not authoritative. These values are demonstration inputs, not claims about a retailer, manufacturer, or live reporting period.

#### TrayConfiguration

- `outer_width`
- `outer_height`
- `outer_depth`
- `front_lip_height`
- `facings_x`
- `units_deep`

The three outer dimensions are `Length` values describing the complete loaded tray footprint, including its products and packaging. Transport adapters expose them as `outer_width_sixteenths`, `outer_height_sixteenths`, and `outer_depth_sixteenths`; they expose the lip as `front_lip_height_sixteenths`. The front-lip height is additional presentation metadata and must be positive and no greater than the loaded outer height. Catalog-configured `facings_x` and `units_deep` are positive preset counts; the current slice has no vertical stacking inside a tray, so units per tray are derived as `facings_x × units_deep`.

A product without a tray configuration is stocked loose. A configured product is stocked as one complete loaded tray per placement; there is no loose-versus-tray choice in this slice. The representative catalog configures five products:

| Product | Loaded outer W × H × D (sixteenths) | Front lip | Preset facings | Units deep |
| --- | ---: | ---: | ---: | ---: |
| `jif_creamy_16` | 175 × 80 × 232 | 20 | 3 | 4 |
| `skippy_creamy_16` | 181 × 78 × 240 | 20 | 3 | 4 |
| `peter_pan_creamy_16` | 178 × 78 × 236 | 20 | 3 | 4 |
| `smuckers_natural_16` | 116 × 84 × 172 | 20 | 2 | 3 |
| `justins_classic_16` | 116 × 86 × 172 | 20 | 2 | 3 |

The representative casepacks are internally consistent with their trays: a case contains a whole number of complete trays. Casepack quantity and units per tray remain distinct concepts and must not be treated as universally equal.

#### ProductOrientation

- `product_id`
- `orientation`: `front`, `side`, or `lay_flat`
- Rotated width, height, and depth.
- Whether the orientation is allowed.

Product identity and geometry must never depend on image availability.

### 6.2 Fixture model

#### Fixture

- `id`
- `type`
- `width_sixteenths`
- `height_sixteenths`
- `depth_sixteenths`

#### Section

- `id`
- `fixture_id`
- `sequence`
- Physical bounds.

#### Shelf

- `id`
- `section_id`
- `width_sixteenths`
- `depth_sixteenths`
- `elevation_sixteenths`
- Usable left, right, and vertical bounds.

### 6.3 Planogram model

#### Planogram

- Stable logical identity and descriptive metadata.

#### PlanogramVersion

- `id`
- `planogram_id`
- `version_number`
- `base_version_id`
- `status`: `draft`, `proposed`, `published`, or `archived`
- `revision`
- `created_by`
- Timestamps.

#### Placement

- `id`
- `version_id`
- `product_id`
- `shelf_id`
- `x_sixteenths`
- `orientation`
- `facings_x`
- `facings_y`
- `facings_z`
- `z_order`

Example:

```json
{
  "id": "plc_123",
  "product_id": "item_456",
  "shelf_id": "shelf_2",
  "x_sixteenths": 400,
  "orientation": "front",
  "facings_x": 3,
  "facings_y": 1,
  "facings_z": 4
}
```

#### ChangeSet

- `id`
- Actor type and identity.
- Human-readable reason.
- Base and resulting revisions.
- Ordered operations.
- Validation summary.
- Timestamp.

#### ValidationIssue

- `id`
- Issue type and severity.
- Affected entity IDs.
- Required and available dimensions when applicable.
- Human-readable explanation.
- Suggested remediation when deterministically available.

## 7. Geometry and Validation

### 7.1 Derived placement dimensions

For a loose product:

```text
display width  = oriented product width  × facings_x
display height = oriented product height × facings_y
required depth = oriented product depth  × facings_z
```

For a tray-configured product:

```text
display width  = tray loaded width
display height = tray loaded height
required depth = tray loaded depth
```

One tray is one placement. The loaded tray footprint replaces the loose-product multiplication; the engine must not multiply the loaded tray width by its preset facings a second time. Adding another tray creates another placement and remains subject to the 1/8-inch inter-placement gap.

Rust must resolve these dimensions through one authoritative placement-footprint path used by add, move, preview, validation, distribution, scene generation, capacity reporting, and proposal impact. The UI and WebMCP consume Rust-derived placement views and must not recreate tray geometry or calculate final coordinates.

### 7.2 Required structural checks

The engine must validate:

- Placement begins within the shelf's usable left bound.
- Placement ends within the shelf's usable right bound.
- Distinct placements on the same shelf keep at least 1/8 inch (2 internal units) between their display bounds.
- Display height is within vertical clearance.
- Required depth is within shelf depth.
- Placements do not overlap.
- The selected orientation is allowed.
- A tray configuration has valid loaded dimensions, front-lip height, preset facings, and units-deep values.
- The fixture and shelf support the placement type.
- Referenced products, shelves, sections, and placements exist.
- All physical values and facing counts are valid.

### 7.3 Merchandising checks

The rules framework must be extensible to support:

- Brand blocking and adjacency.
- Category sequencing.
- Minimum and maximum facings.
- Minimum capacity and days of supply.
- Eye-level preference.
- Price progression.
- Private-brand relationships.
- Sales productivity.
- Store assortment eligibility.

Structural errors block a change. Merchandising rules may be blocking or advisory depending on rule configuration.

## 8. Command and Operation Model

### 8.1 Operations

```ts
type PlanogramOperation =
  | AddPlacement
  | RemovePlacement
  | MovePlacement
  | ChangeFacings
  | ChangeOrientation
  | MoveShelf
  | ResizeFixture
  | ReorderPlacements;
```

Operations must include their before and after values so that changes are auditable and reversible.

### 8.2 Command contract

Every mutating command must:

1. Identify a target draft version.
2. Include `expectedRevision`.
3. Resolve semantic references into proposed operations.
4. Validate the complete proposal.
5. Apply all operations atomically or none of them.
6. Increment the revision once.
7. Return a change set, validation result, and render-scene patch.

When an add intent omits facing counts, Rust resolves loose products to one facing in each dimension and tray-configured products to the catalog preset facings and units deep. An explicit facing count that conflicts with a configured tray must fail validation without changing geometry, revision, or history. Callers must never silently coerce an explicit conflict.

### 8.3 Conflict behavior

If the draft changed after a caller read it, return:

```json
{
  "status": "revision_conflict",
  "expected_revision": 82,
  "current_revision": 83
}
```

The caller must reread current state before retrying. The system must not automatically overwrite or merge a stale mutation.

### 8.4 Command results

Commands return one of:

- `applied`
- `validation_failed`
- `revision_conflict`
- `not_found`
- `forbidden`
- `invalid_command`
- `cancelled`

Applied results include the new revision, change set, affected IDs, validation summary, and a compact scene patch.

## 9. WebMCP Integration

### 9.1 Integration rule

WebMCP is a thin adapter over application queries and commands. WebMCP-specific code must not contain geometry, layout, validation, or persistence logic.

Tools operate on the currently open, authenticated editor session. Closing the page ends that live integration.

### 9.2 Initial tool set

1. `get_planogram_context` — current version, revision, fixture dimensions, selection, and summary statistics.
2. `search_products` — search by description, GTIN/UPC, brand, category, or attributes, including tray availability.
3. `get_product` — dimensions, net weight, performance, casepack, optional tray, allowed orientations, image, and merchandising metadata.
4. `get_section` — placements, physical bounds, and available capacity for a section or shelf.
5. `validate_planogram` — structural and merchandising issues.
6. `add_product` — place an item relative to another placement or within a shelf or section.
7. `remove_product` — remove one or more placements.
8. `move_product` — move relative to another product, shelf, section, or physical location.
9. `set_facings` — update horizontal, vertical, or depth facings.
10. `swap_product` — substitute one SKU while attempting to preserve location.
11. `reflow_section` — close gaps and reorder deterministically under declared constraints.
12. `apply_changes` — atomically apply a previously validated proposal.
13. `create_version` — branch the current version.
14. `compare_versions` — summarize additions, removals, moves, orientation changes, and facing changes.
15. `undo_change_set` — reverse an eligible human or agent change set.

Avoid generic CRUD tools such as `update_placement` as the primary agent interface.

### 9.3 Semantic positioning

Tools should prefer relations over raw coordinates:

```json
{
  "placement_id": "plc_cheerios",
  "relation": "right_of",
  "reference_placement_id": "plc_honey_nut",
  "gap_sixteenths": 2,
  "expected_revision": 82
}
```

The engine resolves the final physical coordinate. A requested gap may be larger than the structural minimum, but never smaller than 2 sixteenths (1/8 inch).

### 9.4 Example registration

```ts
await document.modelContext.registerTool({
  name: "planogram.set_facings",
  title: "Set product facings",
  description:
    "Changes facings for a product placement in the currently open draft. " +
    "Validates physical fit before applying the change and does not publish it.",
  inputSchema: {
    type: "object",
    properties: {
      placementId: { type: "string" },
      horizontalFacings: {
        type: "integer",
        minimum: 1,
        maximum: 100
      },
      expectedRevision: { type: "integer", minimum: 0 }
    },
    required: ["placementId", "horizontalFacings", "expectedRevision"]
  },
  execute: async (args, { signal }) =>
    planogramCommands.setFacings({ ...args, signal })
});
```

All tools must provide precise descriptions, strict JSON Schemas, stable IDs, abort-signal handling, and structured error results.

## 10. Editor Experience

### 10.1 Canvas

The primary editor is a 2D front elevation that supports:

- Zoom and pan.
- Shelf and section visualization.
- Product-image or placeholder rendering.
- Single and multi-selection.
- Drag preview and deterministic snapping.
- Rust-owned packed-left, centered, space-between, and space-evenly shelf distribution.
- Collision, overflow, and rule overlays.
- Semantic zoom for dense planograms.
- Keyboard-accessible movement and editing.

Products should not each be represented as full HTML DOM elements. Catalog, property, inspector, and workflow UI remains normal accessible HTML.

Catalog and inspector surfaces expose product depth, net weight, sales and units per store per week, gross-margin rate, casepack quantity, and tray details when configured. Synthetic performance data must retain its source label in product details rather than appearing to be live retailer data.

### 10.2 Accessibility companion

The canvas must have an HTML companion representation that exposes:

- Current selection.
- Product name and placement details.
- Shelf and section location.
- Facing counts and orientation.
- Product depth, net weight, sales and units per store per week, gross-margin rate, and casepack quantity.
- Tray status, loaded dimensions, front-lip height, preset facings, and units deep when configured.
- Validation issues.
- Keyboard commands and available actions.

Canvas interaction alone is not sufficient for core editing tasks.

### 10.3 AI change review

After an AI-assisted edit, show:

- Each operation in plain language.
- Before and after values.
- Space and capacity impact.
- Constraint violations or warnings.
- Actions for Undo, Compare, and Keep Changes.

Agent changes must never be presented as already published.

## 11. Rendering and Performance

### 11.1 JavaScript/Wasm boundary

Use one long-lived Wasm engine. Do not serialize the complete planogram to JSON on every frame.

Prefer:

- Semantic commands entering Rust.
- Compact results returning to TypeScript.
- Scene patches instead of full scene replacement.
- Typed arrays for large transfers.
- Numeric internal IDs in rendering and hit-testing paths.
- Rust-owned spatial indexes.

Pointer movement must not trigger React state updates for every pixel. Transient drag state should update the renderer directly; one semantic command is committed when the drag completes.

### 11.2 Images

The renderer should use one or more texture atlases with:

- Image decoding and normalization.
- Incremental region uploads.
- Placeholders for missing images.
- Eviction of images not recently visible.

### 11.3 Hit testing

CPU hit testing against the authoritative Rust spatial index is the default. GPU picking is deferred until complex or densely overlapping shapes make it necessary.

### 11.4 `wgpu` rendering model

The initial renderer uses instanced product rectangles so large numbers of placements can be drawn with a small number of draw calls. It uses separate passes for:

1. Fixture backgrounds and section bounds.
2. Shelves, pegboards, and structural lines.
3. Instanced product rectangles and the product-image atlas.
4. Selection, drag preview, and snap guides.
5. Collision, overflow, and validation overlays.

Authoritative coordinates remain integer sixteenths of an inch. They are converted to camera-relative GPU values only when instance buffers are built. Zoom level, device-pixel ratio, and floating-point rendering details must not alter stored planogram geometry.

The renderer must start with a conservative WebGPU feature set and explicit browser capability checks. If WebGPU initialization fails, the application must show a clear unsupported-browser state; a second rendering engine is not required for the MVP.

## 12. Persistence and Collaboration

Persist normalized entities, versions, placements, and change sets in PostgreSQL.

Required guarantees:

- Commands are transactionally atomic.
- Version revisions increase monotonically.
- Published versions are immutable.
- Change sets retain actor, reason, and operation detail.
- Undo creates a compensating change set; it does not erase history.
- Authorization is enforced by application APIs, not only by the browser.

Real-time multi-user collaboration is not required for the first release, but revision conflicts must prevent accidental overwrites from concurrent human and agent activity.

## 13. Security and Trust Boundaries

- WebMCP tools act only with the current signed-in user's permissions.
- Tool inputs are untrusted and must pass schema, authorization, and domain validation.
- Stable public IDs must be translated to internal identifiers at the command boundary.
- Tools may edit drafts but must not publish unless a separate, explicit publishing workflow is later defined.
- Tool responses must avoid exposing credentials, internal database details, or data outside the current user's scope.
- An agent's explanation is advisory; only engine-produced measurements and validation results are authoritative.

## 14. Delivery Phases

### Phase 1 — Deterministic foundation

- Rust `planogram-core` entities and identifiers.
- Product, fixture, shelf, version, and placement models.
- Commands, revisions, operations, and change sets.
- Geometry and structural validation.
- Native Rust tests.

### Phase 2 — Browser engine

- Wasm command adapter.
- Long-lived engine lifecycle.
- Compact TypeScript result types.
- Scene snapshots and patches.
- `wgpu` WebGPU surface initialization.
- Basic instanced rectangles, shelves, and camera controls.

### Phase 3 — Visual editor

- React application shell.
- Catalog and inspector UI.
- Rust/`wgpu` front-elevation canvas.
- Product-image texture atlas and incremental GPU uploads.
- Selection, zoom, pan, drag, snapping, and keyboard commands.
- Accessible canvas companion.

### Phase 4 — Persistence and versioning

- PostgreSQL schema and APIs.
- Draft, proposed, and published version workflows.
- Comparison, undo, and audit history.
- Revision-conflict handling.

### Phase 5 — WebMCP

- Read-only context and catalog tools.
- Mutating semantic tools.
- Change preview and review UI.
- Conflict, validation, cancellation, and authorization handling.

### Phase 6 — Merchandising assistance

- Extensible merchandising rules.
- Deterministic section reflow.
- Intent-based assisted workflows.
- Explanations grounded in engine-produced measurements.

### Phase 7 — Advanced performance

- Profiling against target data sizes.
- Worker isolation where needed.
- Texture-atlas tuning.
- Advanced `wgpu` overlays, semantic zoom, and GPU profiling.
- GPU picking only if CPU spatial-index hit testing proves insufficient.

### Phase 8 — Server-side MCP and optimization

- Persistent catalog and batch-operation tools.
- Background optimization jobs.
- Enterprise data integrations.
- Store-specific planogram generation.

## 15. MVP Acceptance Criteria

The MVP is complete when:

1. A user can load a product catalog with dimensions and images.
2. A user can create a fixture with sections and shelves in feet, inches, and fractions to 1/16 inch.
3. A user can add, remove, move, orient, and change facings for products.
4. The same command API is used by pointer, keyboard, form, and WebMCP interactions.
5. The engine blocks collisions, shelf overflow, depth overflow, invalid orientation, and insufficient vertical clearance.
6. Every successful mutation creates one atomic change set and one revision increment.
7. Undo reverses a change through a recorded compensating change set.
8. Published versions cannot be directly mutated.
9. A new draft or proposal can branch from an existing version.
10. Stale commands return a revision conflict without applying changes.
11. The editor visibly previews and summarizes AI changes before they are treated as accepted work.
12. ChatGPT can inspect the open planogram, search products, validate it, and perform the initial semantic edit set through WebMCP.
13. All authoritative geometry remains stable across zoom level, viewport size, and device-pixel ratio.
14. Core editing and selection information is available through an accessible HTML interface.
15. Every representative catalog product exposes exact depth, net weight, fixed-point performance, and casepack data with the synthetic 13-week source label.
16. The five configured tray products add as one loaded tray per placement, use the loaded tray footprint for fit and distribution, and reject explicit facing conflicts atomically.

## 16. Testing Requirements

### Domain tests

- Exact conversion among internal units, inches, and feet.
- Parsing and formatting of whole-inch and fractional-inch values.
- Rejection of values finer than 1/16 inch without silent rounding.
- Exact dimension and facing calculations.
- Exact fixed-point catalog values and no floating-point drift.
- Complete metrics for all 22 representative products and the exact five tray configurations.
- Tray default-facing resolution and atomic rejection of explicit tray-facing conflicts.
- Tray loaded footprints in add, move, preview, validation, distribution, scene, and capacity calculations.
- Boundary inclusion and overflow.
- Collision detection.
- Rejection of product gaps smaller than 1/8 inch.
- Deterministic packed-left, centered, space-between, and space-evenly shelf distribution on the 1/8-inch grid.
- Allowed and disallowed orientations.
- Atomic application and rollback.
- Revision conflicts.
- Change-set inversion and undo.
- Deterministic reflow for identical inputs.
- Version comparison.

### Integration tests

- TypeScript-to-Wasm command serialization.
- Imperial length fields across TypeScript, Wasm, and persistence boundaries.
- Scene patches after each operation type.
- Database transaction and version guarantees.
- WebMCP schemas, cancellation, authorization, and structured errors.
- Human and WebMCP commands producing equivalent domain results.
- Rust-derived placement footprints and tray metadata remaining identical across Wasm, UI, accessible companion, and WebMCP views.

### Browser tests

- Drag, keyboard, inspector, and WebMCP flows.
- Zoom and pan without geometry drift.
- Selection and focus behavior.
- AI change review, compare, keep, and undo actions.
- Accessible companion content and keyboard operation.
- Catalog and inspector disclosure of product metrics, synthetic source, and tray configuration.

### Performance tests

Define target fixture sizes before optimization. Measure initial load, interaction latency, reflow duration, image-atlas behavior, and frame consistency using representative catalogs and planograms.

## 17. Open Product Decisions

The following decisions must be made before their respective implementation phases:

- Supported catalog import formats and required product attributes.
- Initial fixture types beyond shelves.
- Catalog import behavior for measurements finer than 1/16 inch.
- Draft autosave and recovery behavior.
- User, workspace, and planogram authorization model.
- Publication approval roles and workflow.
- Blocking versus advisory merchandising rules.
- Production capacity, days-of-supply, and retailer-performance data sources beyond the synthetic 13-week representative catalog.
- Target maximum visible placements for the MVP.
- Browser and device support policy.

## 18. References

- [OpenAI Developers: WebMCP apps showcase](https://developers.openai.com/showcase?view=webmcp-apps)
- [`wasm-bindgen` guide](https://rustwasm.github.io/docs/wasm-bindgen/)
- [`web-sys` Canvas 2D example](https://rustwasm.github.io/docs/wasm-bindgen/examples/2d-canvas.html)
- [WebGPU overview](https://webgpu.org/)
