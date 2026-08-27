# Planogrammy WebMCP Demo

## The story

ChatGPT does not manipulate pixels or maintain a second fixture model. It reads the planogram already open in the browser, asks the Rust domain engine to resolve and validate a semantic proposal, pauses for a human decision, and then verifies the committed draft.

The strongest proof is one continuous loop:

1. Read and validate the open draft at revision 0.
2. Resolve real catalog products and fixture shelves through site tools.
3. Preview one multi-shelf assortment as a single atomic proposal.
4. Review the ghosted result and resolved operations in the editor.
5. Approve it with the visible **Accept proposal** button.
6. Inspect the durable approval receipt and validate revision 1.

No record is published or persisted. The demo changes only the in-memory draft in the open tab.

## Setup

From the repository root:

```bash
npm run dev -- --port 4173
```

Open `http://127.0.0.1:4173` in a WebGPU-capable browser with WebMCP site tools enabled. Confirm the footer reads **Revision 0 · All changes local** and **Site tools ready**.

## Audience prompt

Use this prompt from an empty draft:

> Inspect and validate the planogram open in this browser. Then propose a peanut-butter leadership set: put Jif Creamy 40 oz and SKIPPY Creamy 40 oz on Shelf 01 with two horizontal facings each; Jif Creamy 16 oz on Shelf 02 with three facings; SKIPPY Creamy 16.3 oz on Shelf 03 with three facings; Smucker's Natural Creamy 16 oz on Shelf 04 with two facings; and Justin's Classic 16 oz on Shelf 05 with two facings. Resolve the product IDs and shelf capacity through the site's tools. Preview the full set as one atomic change with zero-based shelf sequence. Do not apply it. Stop when the proposal is visible so I can review it.

The expected tool sequence is:

```text
get_planogram_context
validate_planogram
search_products / get_product
get_section
preview_changes
```

The key proof point is the pause: revision stays at 0 while the canvas shows proposed positions and the review panel lists the engine-resolved operations and constraint result.

## Human approval

Click **Accept proposal** in the editor. Confirm all of the following are visible:

- the products appear in the WebGPU fixture;
- the footer advances exactly once, from revision 0 to revision 1;
- the approval receipt names the actor as `human`;
- the receipt records the reason, `change_0001`, and the resolved operation count;
- keyboard focus moves to the receipt heading.

This is intentionally different from asking ChatGPT to call `apply_changes`: the human owns the decision boundary while the same Rust command path owns the mutation.

## Post-approval prompt

> Validate the applied draft, inspect section_01, and report the current revision, validation result, placements, and remaining shelf capacity. Do not make another change.

Expected proof:

- `validate_planogram` returns revision 1, `valid: true`, and no issues;
- `get_section` returns the six committed placements with integer-sixteenth geometry;
- the browser console has no errors;
- the receipt remains visible after validation because read-only tools do not disturb editor state.

## Optional reversal

Use the editor's **Undo** button to record a compensating change set. The draft returns to its empty geometry at revision 2; history is preserved rather than deleted. Run `validate_planogram` once more to show the same invariant at the new revision.

## What this demonstrates

- live-page context instead of an exported snapshot;
- semantic product and shelf references instead of screen coordinates;
- Rust-owned layout, physical fit, minimum-gap, and revision validation;
- a read-only proposal before mutation;
- explicit human approval with durable provenance;
- atomic commit, structured errors, validation, and semantic undo through one command architecture.
