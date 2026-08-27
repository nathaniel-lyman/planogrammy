# Planogrammy brand-blocking and performance demo reel

## Final deliverable

- Preferred: `planogrammy-brand-blocking-performance-reel-elevenlabs-eric.mp4`
- ElevenLabs voice: `Eric - Smooth, Trustworthy` using Eleven v3
- 80.05 seconds
- 1280 × 720 at 30 fps
- H.264 video with mono AAC narration
- SHA-256: `f3be8153cb9262be522f266c439f910fb9cfd3d4f4c065ea2652cb1674a70700`

The requested `n8` voice was not present in either the public ElevenLabs Voice Library or this account's saved voices. The original 64.50-second macOS Reed edition remains available as `planogrammy-brand-blocking-performance-reel.mp4`.

## What the reel demonstrates

1. The 22-SKU catalog and its exact product depth, net weight, sales per store per week, units per store per week, gross margin, casepack, and loaded-tray fields.
2. A user brief to fill the 4-foot mod with all 22 products in contiguous brand blocks.
3. A non-mutating 22-addition proposal with Rust-resolved coordinates, tray footprints, the 1/8-inch minimum gap, and no validation issues.
4. Human approval of one atomic change set at revision 0 → 1.
5. A performance brief to remove low performers and spread stronger sellers.
6. A second proposal with 7 removals, 7 stronger additions, and 5 deterministic moves while preserving brand blocks.
7. A capacity-led proposal adding 23 more placements of the strongest sellers and raising every shelf to 92–100% utilization.
8. Human approval at revision 2 → 3, followed by deterministic shelf balancing.
9. The final valid local draft at revision 7 with 45 placements and an empty fixed base deck.

The performance data shown is the app's synthetic representative trailing-13-week demo data, not retailer actuals. All planogram mutations occurred only in the local in-memory browser session. No persistent or live retailer records were touched.

## Verification

- Live WebGPU editor: valid at revision 7 with zero validation issues.
- Final fixture: 45 placements across all six adjustable shelves; base deck remained empty.
- Minimum-gap shelf utilization after the capacity proposal: 93%, 100%, 99%, 98%, 93%, and 92%.
- Browser warnings/errors: none.
- Video: full decode passed with no ffmpeg errors.
- ElevenLabs audio: mean volume `-19.0 dB`, maximum `-1.5 dB`.
- Representative frames: `contact-sheet-elevenlabs-eric.jpg`.
- Reproduction helpers: `render-frames.swift`, `render-reel.sh`, `render-reel-elevenlabs.sh`, `voice/`, and `voice/elevenlabs-eric/`.

No Planogrammy application source files were changed for this reel.
