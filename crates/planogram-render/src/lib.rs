use planogram_core::{Length, PlacementId, ProductId, RenderScene, ScenePatch, ShelfId, ShelfKind};
use serde::{Deserialize, Serialize};

pub use paint::{scene_vertices, Vertex};

/// Screen-space padding kept around the fitted fixture so the canvas heading
/// and help strip never overlap the frame.
const FIT_MARGIN_X: f32 = 56.0;
const FIT_MARGIN_Y: f32 = 124.0;
/// Pointer distance (CSS px) inside which a shelf line wins over the products
/// sitting on it, so full shelves stay grabbable.
const SHELF_GRAB_PRIORITY: f32 = 5.0;
const SHELF_GRAB_TOLERANCE: f32 = 13.0;

/// Device-px surface size for a CSS-px canvas. When the canvas would exceed
/// the GPU's texture limit, the ratio is lowered uniformly so the fixture
/// renders at reduced resolution instead of failing to present at all.
pub fn surface_extent(
    css_width: u32,
    css_height: u32,
    pixel_ratio: f32,
    max_dimension: u32,
) -> (u32, u32) {
    let css_width = css_width.max(1) as f32;
    let css_height = css_height.max(1) as f32;
    let max_dimension = max_dimension.max(1) as f32;
    let ratio = pixel_ratio
        .min(max_dimension / css_width)
        .min(max_dimension / css_height);
    let scale = |css: f32| ((css * ratio).round() as u32).clamp(1, max_dimension as u32);
    (scale(css_width), scale(css_height))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DragPreview {
    pub shelf_id: ShelfId,
    pub elevation: Length,
    start_elevation: Length,
    start_pointer_y: f32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Selection {
    Shelf { id: ShelfId },
    Placement { id: PlacementId },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HitTarget {
    Shelf { id: ShelfId },
    Placement { id: PlacementId, shelf_id: ShelfId },
}

/// Screen rectangle (CSS px) of one placement, for HTML label overlays. The
/// renderer stays text-free; React positions text from this derived view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacementLabel {
    pub id: PlacementId,
    pub product_id: ProductId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderModel {
    pub scene: RenderScene,
    pub proposal_scene: Option<RenderScene>,
    pub proposal_affected_ids: Vec<String>,
    pub camera: Camera,
    pub selected: Option<Selection>,
    pub drag: Option<DragPreview>,
    pub validation_error: Option<String>,
    /// Tint placements by their SKU's days-of-supply band instead of brand.
    pub supply_overlay: bool,
    viewport_width: f32,
    viewport_height: f32,
}

impl RenderModel {
    pub fn new(scene: RenderScene) -> Self {
        Self {
            scene,
            proposal_scene: None,
            proposal_affected_ids: Vec::new(),
            camera: Camera {
                zoom: 1.0,
                pan_x: 0.0,
                pan_y: 0.0,
            },
            selected: None,
            drag: None,
            validation_error: None,
            supply_overlay: false,
            viewport_width: 1.0,
            viewport_height: 1.0,
        }
    }

    pub fn replace_scene(&mut self, scene: RenderScene) {
        self.scene = scene;
        self.selected = None;
        self.drag = None;
        self.validation_error = None;
        self.clear_proposal_preview();
        self.fit();
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.viewport_width = width.max(1.0);
        self.viewport_height = height.max(1.0);
    }

    pub fn viewport(&self) -> (f32, f32) {
        (self.viewport_width, self.viewport_height)
    }

    pub fn fit(&mut self) {
        self.camera = Camera {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        };
    }

    /// Frame the bay containing this shelf without changing committed geometry.
    pub fn focus_bay(&mut self, shelf_id: &ShelfId) -> bool {
        let Some(shelf) = self
            .scene
            .shelves
            .iter()
            .find(|shelf| &shelf.id == shelf_id)
        else {
            return false;
        };
        let bay_width = shelf.width.sixteenths() as f32;
        let bay_center = shelf.x.sixteenths() as f32 + bay_width / 2.0;
        let scale = ((self.viewport_width - FIT_MARGIN_X).max(120.0) / bay_width).min(
            (self.viewport_height - FIT_MARGIN_Y).max(180.0)
                / self.scene.height.sixteenths() as f32,
        );
        let base_scale = self.fit_scale() / self.camera.zoom;
        self.camera.zoom = (scale / base_scale).clamp(0.35, 16.0);
        self.camera.pan_x =
            (self.scene.width.sixteenths() as f32 / 2.0 - bay_center) * self.fit_scale();
        self.camera.pan_y = 0.0;
        true
    }

    pub fn zoom_by(&mut self, factor: f32) {
        let previous_zoom = self.camera.zoom;
        self.camera.zoom = (previous_zoom * factor).clamp(0.35, 16.0);
        // Keep the world point at the viewport center fixed, including after
        // focusing a bay far from the center of a multi-bay fixture.
        let ratio = self.camera.zoom / previous_zoom;
        self.camera.pan_x *= ratio;
        self.camera.pan_y *= ratio;
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.camera.pan_x += dx;
        self.camera.pan_y += dy;
    }

    pub fn apply_patch(&mut self, patch: &ScenePatch) {
        self.clear_proposal_preview();
        self.scene.sku_supply = patch.sku_supply.clone();
        for updated in &patch.shelves {
            if let Some(current) = self
                .scene
                .shelves
                .iter_mut()
                .find(|shelf| shelf.id == updated.id)
            {
                *current = updated.clone();
            }
        }
        self.scene
            .placements
            .retain(|placement| !patch.removed_placement_ids.contains(&placement.id));
        if matches!(
            &self.selected,
            Some(Selection::Placement { id }) if patch.removed_placement_ids.contains(id)
        ) {
            self.selected = None;
        }
        for updated in &patch.placements {
            if let Some(current) = self
                .scene
                .placements
                .iter_mut()
                .find(|placement| placement.id == updated.id)
            {
                *current = updated.clone();
            } else {
                self.scene.placements.push(updated.clone());
            }
        }
        self.scene.revision = patch.revision;
        self.scene
            .shelves
            .sort_by(|a, b| a.elevation.cmp(&b.elevation).then_with(|| a.id.cmp(&b.id)));
        self.validation_error = None;
    }

    pub fn show_proposal_preview(&mut self, scene: RenderScene, affected_ids: Vec<String>) {
        self.proposal_scene = Some(scene);
        self.proposal_affected_ids = affected_ids;
    }

    pub fn clear_proposal_preview(&mut self) {
        self.proposal_scene = None;
        self.proposal_affected_ids.clear();
    }

    fn fit_scale(&self) -> f32 {
        let horizontal =
            (self.viewport_width - FIT_MARGIN_X).max(120.0) / self.scene.width.sixteenths() as f32;
        let vertical = (self.viewport_height - FIT_MARGIN_Y).max(180.0)
            / self.scene.height.sixteenths() as f32;
        horizontal.min(vertical) * self.camera.zoom
    }

    fn origin(&self) -> (f32, f32) {
        let scale = self.fit_scale();
        (
            (self.viewport_width - self.scene.width.sixteenths() as f32 * scale) / 2.0
                + self.camera.pan_x,
            (self.viewport_height + self.scene.height.sixteenths() as f32 * scale) / 2.0
                + self.camera.pan_y,
        )
    }

    pub fn world_to_screen(&self, x: Length, y: Length) -> (f32, f32) {
        let scale = self.fit_scale();
        let (origin_x, base_y) = self.origin();
        (
            origin_x + x.sixteenths() as f32 * scale,
            base_y - y.sixteenths() as f32 * scale,
        )
    }

    /// Screen rectangle of a placement: (left, top, right, bottom).
    fn placement_rect(
        &self,
        placement: &planogram_core::PlacementSceneNode,
    ) -> Option<(f32, f32, f32, f32)> {
        let shelf = self
            .scene
            .shelves
            .iter()
            .find(|shelf| shelf.id == placement.shelf_id)?;
        let (x1, shelf_y) = self.world_to_screen(shelf.x + placement.x, shelf.elevation);
        let (x2, product_top) = self.world_to_screen(
            shelf.x + placement.x + placement.width,
            shelf.elevation + placement.height,
        );
        Some((x1, product_top, x2, shelf_y))
    }

    fn nearest_shelf_within(&self, x: f32, y: f32, tolerance: f32) -> Option<HitTarget> {
        self.scene
            .shelves
            .iter()
            .filter(|shelf| {
                let left = self.world_to_screen(shelf.x, shelf.elevation).0;
                let right = self
                    .world_to_screen(shelf.x + shelf.width, shelf.elevation)
                    .0;
                x >= left && x <= right
            })
            .min_by(|a, b| {
                let ay = self.world_to_screen(Length::ZERO, a.elevation).1;
                let by = self.world_to_screen(Length::ZERO, b.elevation).1;
                (ay - y).abs().partial_cmp(&(by - y).abs()).unwrap()
            })
            .and_then(|shelf| {
                let sy = self.world_to_screen(Length::ZERO, shelf.elevation).1;
                ((sy - y).abs() <= tolerance).then(|| HitTarget::Shelf {
                    id: shelf.id.clone(),
                })
            })
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<HitTarget> {
        let (left, _) = self.world_to_screen(Length::ZERO, Length::ZERO);
        let (right, _) = self.world_to_screen(self.scene.width, Length::ZERO);
        if x < left - 12.0 || x > right + 12.0 {
            return None;
        }

        // A pointer right on the shelf line grabs the shelf even when products
        // sit on it; anywhere else inside a product selects the product.
        if let Some(shelf) = self.nearest_shelf_within(x, y, SHELF_GRAB_PRIORITY) {
            return Some(shelf);
        }

        if let Some(placement) = self.scene.placements.iter().rev().find(|placement| {
            self.placement_rect(placement)
                .is_some_and(|(x1, top, x2, bottom)| x >= x1 && x <= x2 && y >= top && y <= bottom)
        }) {
            return Some(HitTarget::Placement {
                id: placement.id.clone(),
                shelf_id: placement.shelf_id.clone(),
            });
        }

        self.nearest_shelf_within(x, y, SHELF_GRAB_TOLERANCE)
    }

    /// Placements large enough on screen to carry a text label, clipped to the
    /// viewport. Camera-only; never touches authoritative geometry.
    pub fn placement_labels(&self, min_width: f32, min_height: f32) -> Vec<PlacementLabel> {
        self.scene
            .placements
            .iter()
            .filter_map(|placement| {
                let (x1, top, x2, bottom) = self.placement_rect(placement)?;
                let width = x2 - x1;
                let height = bottom - top;
                let visible = x2 > 0.0
                    && x1 < self.viewport_width
                    && bottom > 0.0
                    && top < self.viewport_height;
                (visible && width >= min_width && height >= min_height).then(|| PlacementLabel {
                    id: placement.id.clone(),
                    product_id: placement.product_id.clone(),
                    x: x1,
                    y: top,
                    width,
                    height,
                })
            })
            .collect()
    }

    pub fn select(&mut self, selection: Option<Selection>) {
        self.selected = selection;
    }

    pub fn begin_drag(&mut self, shelf_id: &ShelfId, pointer_y: f32) -> bool {
        let Some(shelf) = self
            .scene
            .shelves
            .iter()
            .find(|shelf| &shelf.id == shelf_id)
        else {
            return false;
        };
        if shelf.kind != ShelfKind::Adjustable {
            return false;
        }
        self.selected = Some(Selection::Shelf {
            id: shelf_id.clone(),
        });
        self.drag = Some(DragPreview {
            shelf_id: shelf_id.clone(),
            elevation: shelf.elevation,
            start_elevation: shelf.elevation,
            start_pointer_y: pointer_y,
        });
        true
    }

    pub fn preview_drag(&mut self, pointer_y: f32) -> Option<Length> {
        let scale = self.fit_scale();
        let drag = self.drag.as_mut()?;
        let delta = ((drag.start_pointer_y - pointer_y) / scale).round() as i32;
        let raw = drag.start_elevation.sixteenths() + delta;
        let snapped = ((raw as f32 / 16.0).round() as i32) * 16;
        drag.elevation = Length::from_sixteenths(snapped);
        Some(drag.elevation)
    }

    pub fn finish_drag(&mut self) -> Option<(ShelfId, Length)> {
        self.drag.take().map(|drag| (drag.shelf_id, drag.elevation))
    }

    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }
}

/// Platform-independent scene painting: turns the render model into a flat
/// list of colored triangles. Everything here is presentation only; the
/// authoritative geometry is read, never written.
mod paint {
    use super::{RenderModel, Selection};
    use bytemuck::{Pod, Zeroable};
    use planogram_core::{
        Length, PackageShape, PlacementSceneNode, ProductId, ShelfKind, StockingMode, SupplyBand,
    };
    use std::collections::HashMap;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
    pub struct Vertex {
        pub position: [f32; 2],
        pub color: [f32; 4],
    }

    type Rgba = [f32; 4];

    const WHITE: Rgba = [1.0, 1.0, 1.0, 1.0];
    const BLACK: Rgba = [0.0, 0.0, 0.0, 1.0];
    const PAPER: Rgba = [0.985, 0.975, 0.95, 1.0];
    const PANEL_TOP: Rgba = [0.925, 0.92, 0.905, 1.0];
    const PANEL_BOTTOM: Rgba = [0.885, 0.88, 0.865, 1.0];
    const FRAME: Rgba = [0.17, 0.19, 0.21, 1.0];
    const FRAME_EDGE: Rgba = [0.30, 0.32, 0.34, 1.0];
    const SHELF_TOP: Rgba = [0.60, 0.62, 0.64, 1.0];
    const SHELF_BOTTOM: Rgba = [0.34, 0.36, 0.38, 1.0];
    const SHELF_HIGHLIGHT: Rgba = [0.86, 0.87, 0.88, 1.0];
    const SELECTED: Rgba = [0.08, 0.35, 0.92, 1.0];
    const INVALID: Rgba = [0.78, 0.16, 0.15, 1.0];
    const PROPOSAL: Rgba = [0.93, 0.55, 0.08, 0.92];
    const PROPOSAL_FILL: Rgba = [0.95, 0.63, 0.16, 0.24];
    const REMOVED: Rgba = [0.78, 0.20, 0.16, 0.9];
    const DRAG_GUIDE: Rgba = [0.94, 0.58, 0.08, 0.9];

    pub(super) fn supply_color(band: SupplyBand) -> Rgba {
        match band {
            SupplyBand::UnderThreeDays => [0.78, 0.14, 0.13, 1.0],
            SupplyBand::UnderSevenDays => [0.95, 0.56, 0.10, 1.0],
            SupplyBand::UnderFourteenDays => [0.18, 0.60, 0.40, 1.0],
            SupplyBand::FourteenDaysOrMore => [0.20, 0.40, 0.76, 1.0],
            SupplyBand::NoDemand => [0.58, 0.60, 0.62, 1.0],
        }
    }

    const SHELF_THICKNESS: f32 = 5.0;
    const BASE_DECK_THICKNESS: f32 = 10.0;
    const SHELF_SHADOW: f32 = 9.0;

    struct Painter {
        vertices: Vec<Vertex>,
        width: f32,
        height: f32,
    }

    impl Painter {
        fn ndc(&self, x: f32, y: f32) -> [f32; 2] {
            [x / self.width * 2.0 - 1.0, 1.0 - y / self.height * 2.0]
        }

        /// Corner colors run top-left, top-right, bottom-right, bottom-left.
        fn quad(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, colors: [Rgba; 4]) {
            if x2 <= x1 || y2 <= y1 {
                return;
            }
            let a = Vertex {
                position: self.ndc(x1, y1),
                color: colors[0],
            };
            let b = Vertex {
                position: self.ndc(x2, y1),
                color: colors[1],
            };
            let c = Vertex {
                position: self.ndc(x2, y2),
                color: colors[2],
            };
            let d = Vertex {
                position: self.ndc(x1, y2),
                color: colors[3],
            };
            self.vertices.extend_from_slice(&[a, b, c, a, c, d]);
        }

        fn rect(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Rgba) {
            self.quad(x1, y1, x2, y2, [color; 4]);
        }

        fn vgradient(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, top: Rgba, bottom: Rgba) {
            self.quad(x1, y1, x2, y2, [top, top, bottom, bottom]);
        }

        fn hgradient(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, left: Rgba, right: Rgba) {
            self.quad(x1, y1, x2, y2, [left, right, right, left]);
        }

        fn dashed_outline(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Rgba) {
            let dash = 7.0;
            let gap = 5.0;
            let mut x = x1;
            while x < x2 {
                let end = (x + dash).min(x2);
                self.rect(x, y1, end, y1 + 2.0, color);
                self.rect(x, y2 - 2.0, end, y2, color);
                x += dash + gap;
            }
            let mut y = y1;
            while y < y2 {
                let end = (y + dash).min(y2);
                self.rect(x1, y, x1 + 2.0, end, color);
                self.rect(x2 - 2.0, y, x2, end, color);
                y += dash + gap;
            }
        }
    }

    fn rgb(color: [u8; 3]) -> Rgba {
        [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            1.0,
        ]
    }

    fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
            a[3],
        ]
    }

    fn lighten(color: Rgba, t: f32) -> Rgba {
        mix(color, WHITE, t)
    }

    fn darken(color: Rgba, t: f32) -> Rgba {
        mix(color, BLACK, t)
    }

    fn alpha(color: Rgba, a: f32) -> Rgba {
        [color[0], color[1], color[2], a]
    }

    fn shade(a: f32) -> Rgba {
        [0.0, 0.0, 0.0, a]
    }

    fn glow(a: f32) -> Rgba {
        [1.0, 1.0, 1.0, a]
    }

    fn fnv1a(text: &str) -> u32 {
        text.bytes().fold(0x811c_9dc5u32, |hash, byte| {
            (hash ^ byte as u32).wrapping_mul(0x0100_0193)
        })
    }

    fn rgb_to_hsl([r, g, b, _]: Rgba) -> (f32, f32, f32) {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < f32::EPSILON {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        } / 6.0;
        (h, s, l)
    }

    fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Rgba {
        if s <= 0.0 {
            return [l, l, l, 1.0];
        }
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let channel = |mut t: f32| {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        [
            channel(h + 1.0 / 3.0),
            channel(h),
            channel(h - 1.0 / 3.0),
            1.0,
        ]
    }

    /// Deterministic per-SKU variation of the brand color, so sibling SKUs of
    /// one brand stay in the family but no longer render as identical blocks.
    pub(super) fn sku_color(brand: [u8; 3], product_id: &str) -> Rgba {
        let hash = fnv1a(product_id);
        let hue_shift = ((hash & 0xff) as f32 / 255.0 - 0.5) * 0.06;
        let light_shift = (((hash >> 8) & 0xff) as f32 / 255.0 - 0.5) * 0.12;
        let (h, s, l) = rgb_to_hsl(rgb(brand));
        hsl_to_rgb(
            (h + hue_shift).rem_euclid(1.0),
            s,
            (l + light_shift).clamp(0.12, 0.88),
        )
    }

    /// The lid or brand-band color. A near-white lid (the cereal catalog's
    /// cream) would wash the band out, so it falls back to a deep body tone.
    fn package_accent(body: Rgba, lid: Rgba) -> Rgba {
        let luminance = 0.2126 * lid[0] + 0.7152 * lid[1] + 0.0722 * lid[2];
        if luminance > 0.82 {
            darken(body, 0.38)
        } else {
            lid
        }
    }

    struct Unit {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    }

    impl Unit {
        fn width(&self) -> f32 {
            self.x2 - self.x1
        }

        fn height(&self) -> f32 {
            self.y2 - self.y1
        }

        /// Enough pixels to show a lid, label and shading rather than a swatch.
        fn detailed(&self) -> bool {
            self.width() >= 7.0 && self.height() >= 10.0
        }

        fn fine(&self) -> bool {
            self.width() >= 18.0 && self.height() >= 24.0
        }
    }

    /// `paper` scales how far the printed label fades toward paper: 1.0 for
    /// brand colors, lower so heat-map bands stay saturated.
    fn paint_jar(p: &mut Painter, u: &Unit, body: Rgba, lid: Rgba, paper: f32) {
        if !u.detailed() {
            p.vgradient(
                u.x1,
                u.y1,
                u.x2,
                u.y2,
                lighten(body, 0.08),
                darken(body, 0.12),
            );
            return;
        }
        let w = u.width();
        let lid_h = (u.height() * 0.15).max(2.0);
        let lid_inset = w * 0.07;
        p.vgradient(
            u.x1 + lid_inset,
            u.y1,
            u.x2 - lid_inset,
            u.y1 + lid_h,
            lighten(lid, 0.18),
            darken(lid, 0.18),
        );
        p.rect(
            u.x1 + lid_inset,
            u.y1,
            u.x2 - lid_inset,
            u.y1 + 1.0,
            glow(0.35),
        );
        let body_top = u.y1 + lid_h;
        p.vgradient(
            u.x1,
            body_top,
            u.x2,
            u.y2,
            lighten(body, 0.10),
            darken(body, 0.14),
        );
        if u.fine() {
            let body_h = u.y2 - body_top;
            let label_top = body_top + body_h * 0.22;
            let label_bottom = body_top + body_h * 0.80;
            p.rect(
                u.x1 + w * 0.06,
                label_top,
                u.x2 - w * 0.06,
                label_bottom,
                mix(body, PAPER, 0.74 * paper),
            );
            let stripe_y = label_top + (label_bottom - label_top) * 0.38;
            p.rect(
                u.x1 + w * 0.12,
                stripe_y,
                u.x2 - w * 0.12,
                stripe_y + (label_bottom - label_top) * 0.16,
                alpha(body, 0.85),
            );
        }
        // Cylinder shading: a soft highlight down the left, shadow on the right.
        p.hgradient(u.x1, body_top, u.x1 + w * 0.22, u.y2, glow(0.22), glow(0.0));
        p.hgradient(
            u.x2 - w * 0.30,
            body_top,
            u.x2,
            u.y2,
            shade(0.0),
            shade(0.26),
        );
        p.vgradient(u.x1, u.y2 - 3.0, u.x2, u.y2, shade(0.0), shade(0.28));
    }

    fn paint_box(p: &mut Painter, u: &Unit, body: Rgba, accent: Rgba, paper: f32) {
        if !u.detailed() {
            p.vgradient(
                u.x1,
                u.y1,
                u.x2,
                u.y2,
                lighten(body, 0.06),
                darken(body, 0.10),
            );
            return;
        }
        let w = u.width();
        let h = u.height();
        let side = w * 0.07;
        let front_right = u.x2 - side;
        p.vgradient(
            u.x1,
            u.y1,
            u.x2,
            u.y2,
            lighten(body, 0.06),
            darken(body, 0.10),
        );
        p.rect(front_right, u.y1, u.x2, u.y2, shade(0.24));
        let band_h = h * 0.16;
        p.vgradient(
            u.x1,
            u.y1,
            front_right,
            u.y1 + band_h,
            lighten(accent, 0.10),
            darken(accent, 0.06),
        );
        if u.fine() {
            let px1 = u.x1 + w * 0.09;
            let px2 = front_right - w * 0.09;
            let py1 = u.y1 + band_h + h * 0.10;
            let py2 = u.y2 - h * 0.10;
            p.vgradient(
                px1,
                py1,
                px2,
                py2,
                mix(body, PAPER, 0.58 * paper),
                mix(body, PAPER, 0.36 * paper),
            );
            p.rect(px1, py2 - (py2 - py1) * 0.18, px2, py2, alpha(accent, 0.6));
        }
        p.rect(u.x1, u.y1, front_right, u.y1 + 1.0, glow(0.30));
        p.vgradient(u.x1, u.y2 - 3.0, u.x2, u.y2, shade(0.0), shade(0.25));
    }

    fn paint_placement(
        p: &mut Painter,
        model: &RenderModel,
        placement: &PlacementSceneNode,
        rect: (f32, f32, f32, f32),
        supply: Option<SupplyBand>,
    ) {
        let (x1, top, x2, bottom) = rect;
        if matches!(
            &model.selected,
            Some(Selection::Placement { id }) if id == &placement.id
        ) {
            p.rect(x1 - 3.0, top - 3.0, x2 + 3.0, bottom + 3.0, SELECTED);
        }
        let brand_body = sku_color(placement.color, &placement.product_id.0);
        // The overlay keeps each package's shape and shading but swaps its
        // brand color for the band, so the fixture reads as a heat map.
        let (body, accent, paper) = match supply {
            Some(band) => {
                let tint = supply_color(band);
                (mix(brand_body, tint, 0.88), darken(tint, 0.32), 0.3)
            }
            None => (
                brand_body,
                package_accent(brand_body, rgb(placement.lid_color)),
                1.0,
            ),
        };
        let cols = placement.facings_x.max(1);
        let rows = if placement.stocking_mode == StockingMode::Tray {
            1
        } else {
            placement.facings_y.max(1)
        };
        let unit_w = (x2 - x1) / cols as f32;
        let unit_h = (bottom - top) / rows as f32;
        let gap = if unit_w < 4.0 {
            0.0
        } else {
            (unit_w * 0.05).clamp(0.5, 2.0)
        };
        for row in 0..rows {
            for col in 0..cols {
                let unit = Unit {
                    x1: x1 + unit_w * col as f32 + gap / 2.0,
                    y1: bottom - unit_h * (row + 1) as f32 + gap,
                    x2: x1 + unit_w * (col + 1) as f32 - gap / 2.0,
                    y2: bottom - unit_h * row as f32,
                };
                match placement.package_shape {
                    PackageShape::Jar => paint_jar(p, &unit, body, accent, paper),
                    PackageShape::Box => paint_box(p, &unit, body, accent, paper),
                }
            }
        }
        if placement.stocking_mode == StockingMode::Tray {
            if let Some(front_lip_height) = placement.tray_front_lip_height {
                let shelf = model
                    .scene
                    .shelves
                    .iter()
                    .find(|shelf| shelf.id == placement.shelf_id);
                if let Some(shelf) = shelf {
                    let (_, lip_top) = model
                        .world_to_screen(shelf.x + placement.x, shelf.elevation + front_lip_height);
                    let lip_top = lip_top.max(top);
                    p.vgradient(x1, lip_top, x2, bottom, shade(0.30), shade(0.44));
                    p.rect(x1, lip_top, x2, lip_top + 1.5, glow(0.62));
                }
            }
        }
    }

    pub fn scene_vertices(model: &RenderModel) -> Vec<Vertex> {
        let (width, height) = model.viewport();
        let mut p = Painter {
            vertices: Vec::new(),
            width,
            height,
        };
        let (_, base_y) = model.world_to_screen(Length::ZERO, Length::ZERO);
        let (_, top_y) = model.world_to_screen(model.scene.width, model.scene.height);

        // Each base deck identifies a physical bay: back panel, top rail and
        // uprights stay visible when the whole category is fitted.
        for bay in model
            .scene
            .shelves
            .iter()
            .filter(|shelf| shelf.kind == ShelfKind::BaseDeck)
        {
            let bay_left = model.world_to_screen(bay.x, Length::ZERO).0;
            let bay_right = model.world_to_screen(bay.x + bay.width, Length::ZERO).0;
            p.vgradient(bay_left, top_y, bay_right, base_y, PANEL_TOP, PANEL_BOTTOM);
            p.rect(bay_left, top_y - 1.0, bay_right, top_y + 4.0, FRAME);
            for upright in [bay_left, bay_right] {
                p.rect(
                    upright - 2.5,
                    top_y - 1.0,
                    upright + 2.5,
                    base_y + 6.0,
                    FRAME,
                );
                p.rect(
                    upright - 2.5,
                    top_y - 1.0,
                    upright - 1.5,
                    base_y + 6.0,
                    FRAME_EDGE,
                );
            }
        }

        let bands: HashMap<&ProductId, SupplyBand> = if model.supply_overlay {
            model
                .scene
                .sku_supply
                .iter()
                .map(|sku| (&sku.product_id, sku.band))
                .collect()
        } else {
            HashMap::new()
        };
        for placement in &model.scene.placements {
            if let Some(rect) = model.placement_rect(placement) {
                let supply = model.supply_overlay.then(|| {
                    bands
                        .get(&placement.product_id)
                        .copied()
                        .unwrap_or(SupplyBand::NoDemand)
                });
                paint_placement(&mut p, model, placement, rect, supply);
            }
        }

        // Shelves paint after products so the lip sits in front of what rests
        // on it and its shadow falls onto the shelf below.
        for shelf in &model.scene.shelves {
            let elevation = model
                .drag
                .as_ref()
                .filter(|drag| drag.shelf_id == shelf.id)
                .map(|drag| drag.elevation)
                .unwrap_or(shelf.elevation);
            let (left, y) = model.world_to_screen(shelf.x, elevation);
            let right = model.world_to_screen(shelf.x + shelf.width, elevation).0;
            let selected = matches!(
                &model.selected,
                Some(Selection::Shelf { id }) if id == &shelf.id
            );
            let highlight = if model.validation_error.is_some() && selected {
                Some(INVALID)
            } else if selected {
                Some(SELECTED)
            } else {
                None
            };
            if shelf.kind == ShelfKind::BaseDeck {
                let half = BASE_DECK_THICKNESS / 2.0;
                p.vgradient(left, y - half, right, y + half, FRAME_EDGE, FRAME);
                p.rect(left, y - half, right, y - half + 1.0, SHELF_HIGHLIGHT);
                if let Some(color) = highlight {
                    p.rect(left, y - half, right, y - half + 2.0, color);
                }
                continue;
            }
            let half = SHELF_THICKNESS / 2.0;
            p.vgradient(
                left,
                y + half,
                right,
                y + half + SHELF_SHADOW,
                shade(0.16),
                shade(0.0),
            );
            match highlight {
                Some(color) => {
                    p.vgradient(
                        left,
                        y - half - 1.0,
                        right,
                        y + half + 1.0,
                        lighten(color, 0.12),
                        darken(color, 0.12),
                    );
                }
                None => {
                    p.vgradient(left, y - half, right, y + half, SHELF_TOP, SHELF_BOTTOM);
                    p.rect(left, y - half, right, y - half + 1.0, SHELF_HIGHLIGHT);
                }
            }
        }

        if let Some(proposal_scene) = &model.proposal_scene {
            for placement in proposal_scene
                .placements
                .iter()
                .filter(|placement| model.proposal_affected_ids.contains(&placement.id.0))
            {
                let Some(shelf) = proposal_scene
                    .shelves
                    .iter()
                    .find(|shelf| shelf.id == placement.shelf_id)
                else {
                    continue;
                };
                let (x1, shelf_y) = model.world_to_screen(shelf.x + placement.x, shelf.elevation);
                let (x2, product_top) = model.world_to_screen(
                    shelf.x + placement.x + placement.width,
                    shelf.elevation + placement.height,
                );
                p.rect(
                    x1 + 2.0,
                    product_top + 2.0,
                    x2 - 2.0,
                    shelf_y - 2.0,
                    PROPOSAL_FILL,
                );
                p.dashed_outline(
                    x1 - 2.0,
                    product_top - 2.0,
                    x2 + 2.0,
                    shelf_y + 2.0,
                    PROPOSAL,
                );
            }
            for placement in model.scene.placements.iter().filter(|placement| {
                model.proposal_affected_ids.contains(&placement.id.0)
                    && !proposal_scene
                        .placements
                        .iter()
                        .any(|candidate| candidate.id == placement.id)
            }) {
                if let Some((x1, top, x2, bottom)) = model.placement_rect(placement) {
                    p.dashed_outline(x1 - 2.0, top - 2.0, x2 + 2.0, bottom + 2.0, REMOVED);
                }
            }
        }

        if let Some((drag, shelf)) = model.drag.as_ref().and_then(|drag| {
            model
                .scene
                .shelves
                .iter()
                .find(|shelf| shelf.id == drag.shelf_id)
                .map(|shelf| (drag, shelf))
        }) {
            let (left, y) = model.world_to_screen(shelf.x, drag.elevation);
            let right = model
                .world_to_screen(shelf.x + shelf.width, drag.elevation)
                .0;
            let center = (left + right) / 2.0;
            p.rect(center - 0.75, top_y, center + 0.75, base_y, DRAG_GUIDE);
            p.rect(right + 12.0, y - 1.0, right + 44.0, y + 1.0, DRAG_GUIDE);
        }

        p.vertices
    }
}

#[cfg(target_arch = "wasm32")]
mod webgpu {
    use super::*;
    use wasm_bindgen::JsCast;
    use wgpu::util::DeviceExt;

    pub struct WebGpuRenderer {
        pub model: RenderModel,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        pipeline: wgpu::RenderPipeline,
        /// CSS px → device px. The model and hit testing stay in CSS px; only
        /// the surface is allocated at device resolution so lines stay crisp.
        pixel_ratio: f32,
    }

    impl WebGpuRenderer {
        pub async fn new(canvas_id: &str, scene: RenderScene) -> Result<Self, String> {
            console_error_panic_hook::set_once();
            let window = web_sys::window().ok_or("window unavailable")?;
            let document = window.document().ok_or("document unavailable")?;
            let canvas = document
                .get_element_by_id(canvas_id)
                .ok_or("canvas not found")?
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|_| "element is not a canvas")?;
            let pixel_ratio = (window.device_pixel_ratio() as f32).clamp(1.0, 4.0);
            let css_width = canvas.client_width().max(1) as u32;
            let css_height = canvas.client_height().max(1) as u32;
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::BROWSER_WEBGPU,
                ..Default::default()
            });
            let surface = instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .map_err(|error| error.to_string())?;
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|error| format!("WebGPU is unavailable in this browser: {error}"))?;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("planogram device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                })
                .await
                .map_err(|error| error.to_string())?;
            let (width, height) = surface_extent(
                css_width,
                css_height,
                pixel_ratio,
                device.limits().max_texture_dimension_2d,
            );
            let capabilities = surface.get_capabilities(&adapter);
            let format = capabilities
                .formats
                .iter()
                .copied()
                .find(wgpu::TextureFormat::is_srgb)
                .unwrap_or(capabilities.formats[0]);
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: capabilities.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("planogram shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("planogram layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("planogram pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
            let mut renderer = Self {
                model: RenderModel::new(scene),
                surface,
                device,
                queue,
                config,
                pipeline,
                pixel_ratio,
            };
            renderer.model.resize(css_width as f32, css_height as f32);
            renderer.render()?;
            Ok(renderer)
        }

        /// `width` and `height` are CSS px; the surface is sized in device px.
        pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
            (self.config.width, self.config.height) = surface_extent(
                width,
                height,
                self.pixel_ratio,
                self.device.limits().max_texture_dimension_2d,
            );
            self.surface.configure(&self.device, &self.config);
            self.model.resize(width as f32, height as f32);
            self.render()
        }

        pub fn render(&mut self) -> Result<(), String> {
            let vertices = scene_vertices(&self.model);
            let vertex_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("planogram vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let frame = self
                .surface
                .get_current_texture()
                .map_err(|error| error.to_string())?;
            let view = frame.texture.create_view(&Default::default());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("planogram encoder"),
                });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("planogram pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.965,
                                g: 0.962,
                                b: 0.948,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
            self.queue.submit(Some(encoder.finish()));
            frame.present();
            Ok(())
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use webgpu::WebGpuRenderer;

#[cfg(not(target_arch = "wasm32"))]
pub struct WebGpuRenderer {
    pub model: RenderModel,
}

#[cfg(not(target_arch = "wasm32"))]
impl WebGpuRenderer {
    pub async fn new(_canvas_id: &str, scene: RenderScene) -> Result<Self, String> {
        Ok(Self {
            model: RenderModel::new(scene),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.model.resize(width as f32, height as f32);
        Ok(())
    }

    pub fn render(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planogram_core::{CommandResult, DraftVersion, ProductId, SupplyBand, VersionId};

    fn multi_bay_scene(bay_count: i32) -> RenderScene {
        let mut draft = DraftVersion::default();
        let result = draft.add_placement(
            &draft.id.clone(),
            &ProductId::new("jif_creamy_16"),
            &ShelfId::new("shelf_01"),
            0,
            "renderer test setup",
        );
        assert!(matches!(result, CommandResult::Applied { .. }));
        let mut scene = draft.render_scene();
        let first_bay = scene.shelves.clone();
        let bay_width = scene.width;
        for bay in 1..bay_count {
            scene
                .shelves
                .extend(first_bay.iter().cloned().map(|mut shelf| {
                    shelf.id = ShelfId::new(format!("{}_bay_{bay}", shelf.id.0));
                    shelf.x = Length::from_sixteenths(bay_width.sixteenths() * bay);
                    shelf
                }));
        }
        scene.width = Length::from_sixteenths(bay_width.sixteenths() * bay_count);
        scene
    }

    #[test]
    fn supply_overlay_recolors_placements_from_patched_bands_without_moving_geometry() {
        let mut model = RenderModel::new(multi_bay_scene(1));
        model.resize(800.0, 600.0);
        model.fit();
        let positions =
            |vertices: &[Vertex]| vertices.iter().map(|v| v.position).collect::<Vec<_>>();
        let brand = scene_vertices(&model);

        model.supply_overlay = true;
        let healthy = scene_vertices(&model);
        assert_eq!(
            model.scene.sku_supply[0].band,
            SupplyBand::UnderFourteenDays
        );
        assert_eq!(positions(&healthy), positions(&brand));
        assert_ne!(healthy, brand);

        let mut short = model.scene.sku_supply.clone();
        short[0].band = SupplyBand::UnderThreeDays;
        model.apply_patch(&ScenePatch {
            revision: model.scene.revision,
            shelves: Vec::new(),
            placements: Vec::new(),
            removed_placement_ids: Vec::new(),
            sku_supply: short,
            validation: Default::default(),
        });
        let critical = scene_vertices(&model);
        assert_eq!(positions(&critical), positions(&brand));
        assert_ne!(critical, healthy);

        model.supply_overlay = false;
        assert_eq!(scene_vertices(&model), brand);
    }

    #[test]
    fn surface_extent_lowers_ratio_uniformly_past_the_texture_limit() {
        assert_eq!(surface_extent(443, 900, 2.0, 8192), (886, 1800));
        // A canvas stretched to 12,806 CSS px would need a 25,612 px texture.
        assert_eq!(surface_extent(443, 12_806, 2.0, 8192), (283, 8192));
        assert_eq!(surface_extent(0, 0, 2.0, 8192), (2, 2));
    }

    #[test]
    fn same_elevation_shelves_are_picked_in_their_own_bay() {
        let mut renderer = RenderModel::new(multi_bay_scene(8));
        renderer.resize(1200.0, 800.0);
        for bay in 0..8 {
            let id = if bay == 0 {
                ShelfId::new("shelf_01")
            } else {
                ShelfId::new(format!("shelf_01_bay_{bay}"))
            };
            let shelf = renderer
                .scene
                .shelves
                .iter()
                .find(|shelf| shelf.id == id)
                .unwrap();
            // The right side is clear of the setup product in the first bay.
            let (x, y) = renderer.world_to_screen(
                shelf.x + Length::from_sixteenths(shelf.width.sixteenths() - 32),
                shelf.elevation,
            );
            assert_eq!(renderer.hit_test(x, y), Some(HitTarget::Shelf { id }));
        }
    }

    #[test]
    fn cross_bay_patch_keeps_local_coordinates_and_updates_world_hit_target() {
        let mut renderer = RenderModel::new(multi_bay_scene(2));
        renderer.resize(1200.0, 800.0);
        let mut moved = renderer.scene.placements[0].clone();
        let source = renderer
            .scene
            .shelves
            .iter()
            .find(|shelf| shelf.id == moved.shelf_id)
            .unwrap()
            .clone();
        let destination = renderer
            .scene
            .shelves
            .iter()
            .find(|shelf| shelf.id == ShelfId::new("shelf_01_bay_1"))
            .unwrap()
            .clone();
        let local_midpoint = Length::from_sixteenths(moved.width.sixteenths() / 2);
        let product_midpoint_y =
            source.elevation + Length::from_sixteenths(moved.height.sixteenths() / 2);
        let (old_x, y) = renderer.world_to_screen(source.x + local_midpoint, product_midpoint_y);
        renderer.select(Some(Selection::Placement {
            id: moved.id.clone(),
        }));
        moved.shelf_id = destination.id.clone();
        moved.x = Length::from_sixteenths(2);
        renderer.apply_patch(&ScenePatch {
            revision: renderer.scene.revision + 1,
            shelves: Vec::new(),
            placements: vec![moved.clone()],
            removed_placement_ids: Vec::new(),
            sku_supply: Vec::new(),
            validation: Default::default(),
        });
        let (new_x, _) =
            renderer.world_to_screen(destination.x + moved.x + local_midpoint, product_midpoint_y);
        assert_eq!(renderer.hit_test(old_x, y), None);
        assert_eq!(
            renderer.hit_test(new_x, y),
            Some(HitTarget::Placement {
                id: moved.id.clone(),
                shelf_id: destination.id,
            })
        );
        assert_eq!(renderer.scene.placements, vec![moved.clone()]);
        assert_eq!(
            renderer.selected,
            Some(Selection::Placement { id: moved.id })
        );
    }

    #[test]
    fn bay_focus_centers_the_requested_bay_without_mutating_scene() {
        let scene = multi_bay_scene(8);
        let mut renderer = RenderModel::new(scene.clone());
        renderer.resize(1200.0, 800.0);
        let fit_all_scale = renderer.fit_scale();
        assert!(renderer.focus_bay(&ShelfId::new("shelf_01_bay_6")));
        let shelf = renderer
            .scene
            .shelves
            .iter()
            .find(|shelf| shelf.id == ShelfId::new("shelf_01_bay_6"))
            .unwrap();
        let (left, _) = renderer.world_to_screen(shelf.x, Length::ZERO);
        let (right, _) = renderer.world_to_screen(shelf.x + shelf.width, Length::ZERO);
        assert!(((left + right) / 2.0 - 600.0).abs() < 0.01);
        assert!(left >= 0.0 && right <= 1200.0);
        assert!(renderer.fit_scale() > fit_all_scale);
        let bay_center = shelf.x + Length::from_sixteenths(shelf.width.sixteenths() / 2);
        let focused = renderer.camera;
        assert!(!renderer.focus_bay(&ShelfId::new("missing")));
        assert_eq!(renderer.camera, focused);
        renderer.zoom_by(1.5);
        assert!((renderer.world_to_screen(bay_center, Length::ZERO).0 - 600.0).abs() < 0.01);
        renderer.pan_by(50.0, -40.0);
        assert_eq!(renderer.scene, scene);
        renderer.fit();
        assert!((renderer.fit_scale() - fit_all_scale).abs() < f32::EPSILON);
        assert_eq!(renderer.scene, scene);
    }

    #[test]
    fn camera_changes_do_not_mutate_authoritative_geometry() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let result = draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &ShelfId::new("shelf_01"),
            0,
            "test add",
        );
        assert!(matches!(result, CommandResult::Applied { .. }));
        let scene = draft.render_scene();
        let shelf_values = scene
            .shelves
            .iter()
            .map(|shelf| shelf.elevation)
            .collect::<Vec<_>>();
        let placement_values = scene
            .placements
            .iter()
            .map(|placement| (placement.shelf_id.clone(), placement.x))
            .collect::<Vec<_>>();
        let mut renderer = RenderModel::new(scene);
        renderer.resize(900.0, 700.0);
        renderer.zoom_by(1.4);
        renderer.pan_by(80.0, -20.0);
        renderer.resize(1800.0, 1400.0);
        assert_eq!(
            renderer
                .scene
                .shelves
                .iter()
                .map(|shelf| shelf.elevation)
                .collect::<Vec<_>>(),
            shelf_values
        );
        assert_eq!(
            renderer
                .scene
                .placements
                .iter()
                .map(|placement| (placement.shelf_id.clone(), placement.x))
                .collect::<Vec<_>>(),
            placement_values
        );
    }

    #[test]
    fn proposal_preview_is_non_mutating_and_clears_on_patch() {
        let mut draft = DraftVersion::default();
        let original = draft.render_scene();
        let version = draft.id.clone();
        let result = draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &ShelfId::new("shelf_01"),
            0,
            "test proposal",
        );
        let CommandResult::Applied {
            affected_ids,
            scene_patch,
            ..
        } = result
        else {
            panic!("placement should apply");
        };
        let proposed = draft.render_scene();
        let mut renderer = RenderModel::new(original.clone());
        renderer.show_proposal_preview(proposed, affected_ids);
        assert_eq!(renderer.scene, original);
        assert_eq!(
            renderer.proposal_scene.as_ref().unwrap().placements.len(),
            1
        );

        renderer.apply_patch(&scene_patch);
        assert!(renderer.proposal_scene.is_none());
        assert_eq!(renderer.scene.placements.len(), 1);
    }

    #[test]
    fn pointer_preview_snaps_to_whole_inches_and_base_deck_cannot_drag() {
        let draft = DraftVersion::default();
        let mut renderer = RenderModel::new(draft.render_scene());
        renderer.resize(900.0, 700.0);
        assert!(!renderer.begin_drag(&ShelfId::new("base_deck"), 300.0));
        assert!(renderer.begin_drag(&ShelfId::new("shelf_01"), 300.0));
        let scale = renderer.fit_scale();
        let preview = renderer.preview_drag(300.0 - scale * 9.0).unwrap();
        assert_eq!(preview.sixteenths(), 208);
    }

    #[test]
    fn placements_win_hit_testing_and_removed_selection_is_cleared() {
        let mut draft = DraftVersion::default();
        let result = draft.add_placement(
            &VersionId::new("version_draft_01"),
            &ProductId::new("jif_creamy_16"),
            &ShelfId::new("shelf_01"),
            0,
            "test add",
        );
        assert!(matches!(result, CommandResult::Applied { .. }));
        let mut renderer = RenderModel::new(draft.render_scene());
        renderer.resize(1_000.0, 800.0);
        let placement = renderer.scene.placements[0].clone();
        let shelf = renderer
            .scene
            .shelves
            .iter()
            .find(|shelf| shelf.id == placement.shelf_id)
            .unwrap();
        let (left, shelf_y) = renderer.world_to_screen(shelf.x + placement.x, shelf.elevation);
        let (right, top) = renderer.world_to_screen(
            shelf.x + placement.x + placement.width,
            shelf.elevation + placement.height,
        );
        let hit = renderer.hit_test((left + right) / 2.0, (top + shelf_y) / 2.0);
        assert_eq!(
            hit,
            Some(HitTarget::Placement {
                id: placement.id.clone(),
                shelf_id: placement.shelf_id.clone(),
            })
        );

        renderer.select(Some(Selection::Placement {
            id: placement.id.clone(),
        }));
        let mut moved = placement.clone();
        moved.shelf_id = ShelfId::new("shelf_02");
        moved.x = Length::from_sixteenths(2);
        renderer.apply_patch(&ScenePatch {
            revision: 2,
            shelves: Vec::new(),
            placements: vec![moved.clone()],
            removed_placement_ids: Vec::new(),
            sku_supply: Vec::new(),
            validation: Default::default(),
        });
        assert_eq!(
            renderer.selected,
            Some(Selection::Placement {
                id: placement.id.clone()
            })
        );
        assert_eq!(
            renderer.scene.placements[0].shelf_id,
            ShelfId::new("shelf_02")
        );
        assert_eq!(renderer.scene.placements[0].x, Length::from_sixteenths(2));

        renderer.apply_patch(&ScenePatch {
            revision: 3,
            shelves: Vec::new(),
            placements: Vec::new(),
            removed_placement_ids: vec![placement.id],
            sku_supply: Vec::new(),
            validation: Default::default(),
        });
        assert!(renderer.scene.placements.is_empty());
        assert_eq!(renderer.selected, None);
    }

    #[test]
    fn shelf_line_wins_over_products_resting_on_it() {
        let mut draft = DraftVersion::default();
        let result = draft.add_placement(
            &draft.id.clone(),
            &ProductId::new("jif_creamy_16"),
            &ShelfId::new("shelf_01"),
            0,
            "test add",
        );
        assert!(matches!(result, CommandResult::Applied { .. }));
        let mut renderer = RenderModel::new(draft.render_scene());
        renderer.resize(1_000.0, 800.0);
        let placement = renderer.scene.placements[0].clone();
        let shelf = renderer
            .scene
            .shelves
            .iter()
            .find(|shelf| shelf.id == placement.shelf_id)
            .unwrap();
        let (left, shelf_y) = renderer.world_to_screen(shelf.x + placement.x, shelf.elevation);
        let (right, top) = renderer.world_to_screen(
            shelf.x + placement.x + placement.width,
            shelf.elevation + placement.height,
        );
        let x = (left + right) / 2.0;
        assert_eq!(
            renderer.hit_test(x, shelf_y - 2.0),
            Some(HitTarget::Shelf {
                id: shelf.id.clone()
            })
        );
        assert_eq!(
            renderer.hit_test(x, shelf_y - SHELF_GRAB_PRIORITY - 4.0),
            Some(HitTarget::Placement {
                id: placement.id.clone(),
                shelf_id: placement.shelf_id.clone(),
            })
        );
        assert!(top < shelf_y - SHELF_GRAB_PRIORITY - 4.0);
    }

    #[test]
    fn labels_follow_the_camera_and_respect_minimum_size() {
        let mut draft = DraftVersion::default();
        let result = draft.add_placement(
            &draft.id.clone(),
            &ProductId::new("jif_creamy_16"),
            &ShelfId::new("shelf_01"),
            0,
            "test add",
        );
        assert!(matches!(result, CommandResult::Applied { .. }));
        let mut renderer = RenderModel::new(draft.render_scene());
        renderer.resize(1_000.0, 800.0);
        let labels = renderer.placement_labels(0.0, 0.0);
        assert_eq!(labels.len(), 1);
        let placement = &renderer.scene.placements[0];
        let (x1, top, x2, bottom) = renderer.placement_rect(placement).unwrap();
        assert_eq!(labels[0].id, placement.id);
        assert_eq!(labels[0].product_id, placement.product_id);
        assert!((labels[0].x - x1).abs() < 0.01 && (labels[0].y - top).abs() < 0.01);
        assert!((labels[0].width - (x2 - x1)).abs() < 0.01);
        assert!((labels[0].height - (bottom - top)).abs() < 0.01);
        assert!(renderer.placement_labels(10_000.0, 0.0).is_empty());
        renderer.pan_by(-5_000.0, 0.0);
        assert!(renderer.placement_labels(0.0, 0.0).is_empty());
        let scene_before = renderer.scene.clone();
        renderer.zoom_by(3.0);
        let zoomed = renderer.placement_labels(0.0, 0.0);
        assert!(zoomed.is_empty() || zoomed[0].width > labels[0].width);
        assert_eq!(renderer.scene, scene_before);
    }

    #[test]
    fn sku_colors_are_deterministic_and_vary_within_a_brand() {
        let a = paint::sku_color([207, 31, 38], "jif_creamy_16");
        let again = paint::sku_color([207, 31, 38], "jif_creamy_16");
        let sibling = paint::sku_color([207, 31, 38], "jif_crunchy_16");
        assert_eq!(a, again);
        assert_ne!(a, sibling);
        for channel in a.iter().chain(sibling.iter()) {
            assert!((0.0..=1.0).contains(channel));
        }
        let draft = DraftVersion::default();
        let mut renderer = RenderModel::new(draft.render_scene());
        renderer.resize(1_000.0, 800.0);
        assert!(!scene_vertices(&renderer).is_empty());
    }
}
