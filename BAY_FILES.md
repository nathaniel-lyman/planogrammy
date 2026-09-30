# Portable bay files, version 1

Use **Save bay**, enter a name and choose **Download bay**. Keep the downloaded `.planogrammy.json` file. Later, use **Open bay** in a new editor tab to continue editing and undoing earlier changes. Saving always downloads a new copy; it does not overwrite an earlier download or autosave. A browser can cancel or block a download after the page starts it, so confirm the file is in your downloads before closing.

The document name identifies the arrangement; the fixture retains its `4' Standard Bay` identity. The file contains `format: "planogrammy-bay"`, `format_version: 1`, `name`, and the serialized committed `draft`. Every geometry field explicitly uses `_sixteenths`, including shelf-move before/after elevations. Catalog metrics retain their fixed-point units. The draft includes the complete catalog snapshot, placements, stable IDs, revisions, ID counters, and all change sets including compensating undo records.

Pending proposals are never exported or accepted by saving. The Save dialog explains this when a proposal is pending. Open asks before discarding unsaved revisions or a proposal; cancelling keeps both. Browser navigation also warns while either exists. Selection, pan, zoom, renderer previews and approval receipts reset on successful open.

The Wasm transport parses the file, rejects unsupported versions and unknown/incomplete fields, then asks Rust to replay the audit trail through the existing command validators. The resulting draft must equal the saved snapshot exactly. Restore verifies the current expected revision and only then replaces the active state and rendered scene. Failed reads, malformed files, stale revisions and cancelled replacements leave the current bay intact. File contents are local data, never executable code or site-tool instructions.

Version 1 supports the existing standard bay with moved shelves and the saved catalog, not arbitrary fixture import. Limits are 8 MiB per file, 100 name characters, 10,000 change sets, 1,000 catalog entries and 1,000 operations per change set. Catalog dimensions are bounded to 1,000 inches before geometry math; fixed-point performance must be safely representable by JavaScript integers. Files carry no authenticity signature: validation establishes a consistent editable history, not the real-world identity of an actor. Future incompatible schema or command semantics require an explicit format migration/version change.

There is no cloud storage, autosave, login, PSA import or publication in this slice.

## Version 2: cereal scenarios

**Cereal challenge** creates the seeded synthetic eight-to-six exercise. Save includes the immutable eight-bay baseline, generator seed/version and explicit assumptions, all six-bay alternatives and their normal domain histories, and the selected view. Camera/selection and pending proposals are excluded. The version-2 envelope replaces `draft` with `document`; standard single-bay downloads remain version 1. Both versions open in this editor. Older editors safely reject version 2.

Opening regenerates the versioned genesis, validates the baseline and six-bay constraint, replays each alternative's history and compares every exact snapshot. A monotonic in-session document revision guards replacement and dirty tracking across alternative switches. Switching views keeps committed edits in their alternative; duplicating copies its complete history under a fresh version ID. Pending proposal discard requires confirmation. Comparison formulas and limitations are visible under **Assumptions & SKU comparison**, and the turnover sum is labeled separately from category inventory turns or replenishment trips.
