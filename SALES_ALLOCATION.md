# Sales-based allocation

Sales allocation redistributes the horizontal facings of products already on a shelf or in a bay. The inputs are synthetic planning assumptions. Changing facings changes shelf capacity, never the assumed revenue or unit demand.

## Try it locally

1. Run `npm run build`, then `npm run dev`, and open `http://127.0.0.1:4173` in a browser with WebGPU.
2. Start **Cereal challenge** with seed `20260930`. The 100 fictional SKUs include different prices, unit demand, and pack widths. The eight-bay baseline stays locked; the six-bay target is editable.
3. Select an occupied shelf in the fixture outline. Choose **Sales allocation**, then the contribution basis, allocation measure, scope, and facing limits.
4. Preview and review each SKU's contribution and before/after allocation. Accept the proposal to record one change set, or reject it to leave the plan unchanged.
5. Undo restores exact previous positions and facing counts. Save bay downloads the committed arrangement and history. Pending previews are excluded. Open bay validates and restores the saved file, including its undo history.

## Choices and retained constraints

| Choice | Meaning |
| --- | --- |
| Revenue contribution | Weight each represented SKU by synthetic sales cents per store per week. |
| Unit contribution | Weight each represented SKU by synthetic units per store per week, stored as milliunits. |
| Shelf space | Allocate horizontal display width; wider packs consume more of a SKU's share. Gaps and unused space are excluded from SKU shares. |
| Facings | Allocate horizontal facing counts, irrespective of pack width. |
| Selected shelf | Compare represented SKUs on one adjustable shelf. |
| Selected bay | Compare represented SKUs across the bay's occupied adjustable shelves. Products stay on their existing shelves. |
| Minimum / maximum | Bounds for every existing loose placement, from 1 through 100 horizontal facings. Loaded trays keep their catalog presets. |

The represented assortment, placement IDs, left-to-right order, shelf assignments, vertical facings, and depth facings stay fixed. No products are added or removed. Minimum representation is therefore mandatory. Empty shelves and base decks do not receive new placements. Products outside the selected scope are unaffected. Duplicate placements of a SKU share one contribution weight across the scope.

## Deterministic allocation

Rust starts each loose placement at the requested minimum and retains fixed trays. It then adds one physically fitting horizontal facing at a time, prioritizing the SKU with the smallest prospective total allocation divided by its contribution. The chosen measure is display width or horizontal facings. Integer comparisons avoid floating-point layout decisions. Stable shelf elevation/ID and placement position/ID break ties.

Allocation stops when no eligible additional facing fits or all eligible placements reach their maximum. Zero-contribution products keep their required minimum only. Rust distributes residual slack with the existing space-evenly block layout, maintaining the 1/8-inch position grid and minimum gaps. Same-SKU neighbors remain a block.

This is a bounded greedy apportionment heuristic. Fixed trays, minimums, maximums, whole facings, and existing shelf assignments may prevent matching contribution shares exactly. It does not search alternative shelf assignments, optimize assortment, forecast lost sales, or promise a global optimum. The preview exposes the actual result so the user can judge the tradeoff.

## Validation and persistence

Missing, invalid, or inconsistent performance metadata, all-zero contribution, infeasible minimums, physical-fit failures, stale revisions, and locked versions are rejected without changing the draft. Source and period must be nonempty and consistent within the selected scope. The immutable cereal generator's exact per-SKU illustrative-price suffix is treated as metadata within its common synthetic source. Other mixed sources remain invalid. Preview does not mutate committed state. Approval recomputes the semantic request through Rust and validates the complete result before one atomic change set is applied.

The browser stores only the pending semantic request and its report; it never calculates coordinates. Replacing, cancelling, or invalidating a proposal prevents stale approval. Repeating an already-resolved allocation reports that the arrangement already matches and does not add history. Recorded reflows use the existing portable-file format, so version-1 standard bays and version-2 cereal scenarios retain exact Save/Open and compensating undo behavior. The immutable version-1 cereal generator is unchanged.
