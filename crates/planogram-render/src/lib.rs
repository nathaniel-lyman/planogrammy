use planogram_core::{Length, PlacementId, RenderScene, ScenePatch, ShelfId, ShelfKind};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderModel {
    pub scene: RenderScene,
    pub proposal_scene: Option<RenderScene>,
    pub proposal_affected_ids: Vec<String>,
    pub camera: Camera,
    pub selected: Option<Selection>,
    pub drag: Option<DragPreview>,
    pub validation_error: Option<String>,
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
            viewport_width: 1.0,
            viewport_height: 1.0,
        }
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.viewport_width = width.max(1.0);
        self.viewport_height = height.max(1.0);
    }

    pub fn fit(&mut self) {
        self.camera = Camera {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        };
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.camera.zoom = (self.camera.zoom * factor).clamp(0.35, 5.0);
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.camera.pan_x += dx;
        self.camera.pan_y += dy;
    }

    pub fn apply_patch(&mut self, patch: &ScenePatch) {
        self.clear_proposal_preview();
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
            (self.viewport_width - 180.0).max(120.0) / self.scene.width.sixteenths() as f32;
        let vertical =
            (self.viewport_height - 220.0).max(180.0) / self.scene.height.sixteenths() as f32;
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

    pub fn hit_test(&self, x: f32, y: f32) -> Option<HitTarget> {
        let (left, _) = self.world_to_screen(Length::ZERO, Length::ZERO);
        let (right, _) = self.world_to_screen(self.scene.width, Length::ZERO);
        if x < left - 12.0 || x > right + 12.0 {
            return None;
        }

        if let Some(placement) = self.scene.placements.iter().rev().find(|placement| {
            let Some(shelf) = self
                .scene
                .shelves
                .iter()
                .find(|shelf| shelf.id == placement.shelf_id)
            else {
                return false;
            };
            let (x1, shelf_y) = self.world_to_screen(placement.x, shelf.elevation);
            let (x2, product_top) = self.world_to_screen(
                placement.x + placement.width,
                shelf.elevation + placement.height,
            );
            x >= x1 && x <= x2 && y >= product_top && y <= shelf_y
        }) {
            return Some(HitTarget::Placement {
                id: placement.id.clone(),
                shelf_id: placement.shelf_id.clone(),
            });
        }

        self.scene
            .shelves
            .iter()
            .min_by(|a, b| {
                let ay = self.world_to_screen(Length::ZERO, a.elevation).1;
                let by = self.world_to_screen(Length::ZERO, b.elevation).1;
                (ay - y).abs().partial_cmp(&(by - y).abs()).unwrap()
            })
            .and_then(|shelf| {
                let sy = self.world_to_screen(Length::ZERO, shelf.elevation).1;
                ((sy - y).abs() <= 13.0).then(|| HitTarget::Shelf {
                    id: shelf.id.clone(),
                })
            })
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

#[cfg(target_arch = "wasm32")]
mod webgpu {
    use super::*;
    use bytemuck::{Pod, Zeroable};
    use planogram_core::StockingMode;
    use wasm_bindgen::JsCast;
    use wgpu::util::DeviceExt;

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct Vertex {
        position: [f32; 2],
        color: [f32; 4],
    }

    pub struct WebGpuRenderer {
        pub model: RenderModel,
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        pipeline: wgpu::RenderPipeline,
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
            let width = canvas.client_width().max(1) as u32;
            let height = canvas.client_height().max(1) as u32;
            canvas.set_width(width);
            canvas.set_height(height);
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
            };
            renderer.model.resize(width as f32, height as f32);
            renderer.render()?;
            Ok(renderer)
        }

        pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
            self.config.width = width.max(1);
            self.config.height = height.max(1);
            self.surface.configure(&self.device, &self.config);
            self.model.resize(width as f32, height as f32);
            self.render()
        }

        fn rect(
            vertices: &mut Vec<Vertex>,
            width: f32,
            height: f32,
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
            color: [f32; 4],
        ) {
            let ndc = |x: f32, y: f32| [x / width * 2.0 - 1.0, 1.0 - y / height * 2.0];
            let (a, b, c, d) = (ndc(x1, y1), ndc(x2, y1), ndc(x2, y2), ndc(x1, y2));
            for p in [a, b, c, a, c, d] {
                vertices.push(Vertex { position: p, color });
            }
        }

        fn dashed_outline(
            vertices: &mut Vec<Vertex>,
            width: f32,
            height: f32,
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
            color: [f32; 4],
        ) {
            let dash = 7.0;
            let gap = 5.0;
            let mut x = x1;
            while x < x2 {
                let end = (x + dash).min(x2);
                Self::rect(vertices, width, height, x, y1, end, y1 + 2.0, color);
                Self::rect(vertices, width, height, x, y2 - 2.0, end, y2, color);
                x += dash + gap;
            }
            let mut y = y1;
            while y < y2 {
                let end = (y + dash).min(y2);
                Self::rect(vertices, width, height, x1, y, x1 + 2.0, end, color);
                Self::rect(vertices, width, height, x2 - 2.0, y, x2, end, color);
                y += dash + gap;
            }
        }

        pub fn render(&mut self) -> Result<(), String> {
            let mut vertices = Vec::new();
            let w = self.config.width as f32;
            let h = self.config.height as f32;
            let (left, base_y) = self.model.world_to_screen(Length::ZERO, Length::ZERO);
            let (right, top_y) = self
                .model
                .world_to_screen(self.model.scene.width, self.model.scene.height);
            Self::rect(
                &mut vertices,
                w,
                h,
                left - 7.0,
                top_y,
                right + 7.0,
                top_y + 3.0,
                [0.10, 0.12, 0.14, 1.0],
            );
            Self::rect(
                &mut vertices,
                w,
                h,
                left - 7.0,
                base_y - 3.0,
                right + 7.0,
                base_y + 7.0,
                [0.10, 0.12, 0.14, 1.0],
            );
            Self::rect(
                &mut vertices,
                w,
                h,
                left - 6.0,
                top_y,
                left + 1.0,
                base_y,
                [0.18, 0.20, 0.22, 1.0],
            );
            Self::rect(
                &mut vertices,
                w,
                h,
                right - 1.0,
                top_y,
                right + 6.0,
                base_y,
                [0.18, 0.20, 0.22, 1.0],
            );
            for shelf in &self.model.scene.shelves {
                let elevation = self
                    .model
                    .drag
                    .as_ref()
                    .filter(|drag| drag.shelf_id == shelf.id)
                    .map(|drag| drag.elevation)
                    .unwrap_or(shelf.elevation);
                let y = self.model.world_to_screen(Length::ZERO, elevation).1;
                let selected = matches!(
                    &self.model.selected,
                    Some(Selection::Shelf { id }) if id == &shelf.id
                );
                let color = if self.model.validation_error.is_some() && selected {
                    [0.78, 0.16, 0.15, 1.0]
                } else if selected {
                    [0.08, 0.35, 0.92, 1.0]
                } else if shelf.kind == ShelfKind::BaseDeck {
                    [0.12, 0.14, 0.16, 1.0]
                } else {
                    [0.28, 0.30, 0.32, 1.0]
                };
                let thickness = if shelf.kind == ShelfKind::BaseDeck {
                    9.0
                } else if selected {
                    6.0
                } else {
                    4.0
                };
                Self::rect(
                    &mut vertices,
                    w,
                    h,
                    left - 3.0,
                    y - thickness / 2.0,
                    right + 3.0,
                    y + thickness / 2.0,
                    color,
                );
            }
            for placement in &self.model.scene.placements {
                let Some(shelf) = self
                    .model
                    .scene
                    .shelves
                    .iter()
                    .find(|shelf| shelf.id == placement.shelf_id)
                else {
                    continue;
                };
                let (x1, shelf_y) = self.model.world_to_screen(placement.x, shelf.elevation);
                let (x2, product_top) = self.model.world_to_screen(
                    placement.x + placement.width,
                    shelf.elevation + placement.height,
                );
                if matches!(
                    &self.model.selected,
                    Some(Selection::Placement { id }) if id == &placement.id
                ) {
                    Self::rect(
                        &mut vertices,
                        w,
                        h,
                        x1 - 3.0,
                        product_top - 3.0,
                        x2 + 3.0,
                        shelf_y + 3.0,
                        [0.08, 0.35, 0.92, 1.0],
                    );
                }
                Self::rect(
                    &mut vertices,
                    w,
                    h,
                    x1,
                    product_top,
                    x2,
                    shelf_y,
                    [
                        placement.color[0] as f32 / 255.0,
                        placement.color[1] as f32 / 255.0,
                        placement.color[2] as f32 / 255.0,
                        1.0,
                    ],
                );
                if placement.stocking_mode == StockingMode::Tray {
                    let tray_treatment = [0.08, 0.09, 0.10, 0.48];
                    if placement.facings_x > 1 {
                        for facing in 1..placement.facings_x {
                            let separator_x =
                                x1 + (x2 - x1) * facing as f32 / placement.facings_x as f32;
                            Self::rect(
                                &mut vertices,
                                w,
                                h,
                                separator_x - 0.75,
                                product_top + 1.0,
                                separator_x + 0.75,
                                shelf_y - 1.0,
                                tray_treatment,
                            );
                        }
                    }
                    if let Some(front_lip_height) = placement.tray_front_lip_height {
                        let (_, lip_top) = self
                            .model
                            .world_to_screen(placement.x, shelf.elevation + front_lip_height);
                        Self::rect(
                            &mut vertices,
                            w,
                            h,
                            x1,
                            lip_top.max(product_top),
                            x2,
                            shelf_y,
                            tray_treatment,
                        );
                        Self::rect(
                            &mut vertices,
                            w,
                            h,
                            x1,
                            lip_top.max(product_top),
                            x2,
                            lip_top.max(product_top) + 1.5,
                            [0.96, 0.97, 0.98, 0.62],
                        );
                    }
                }
            }
            if let Some(proposal_scene) = &self.model.proposal_scene {
                let proposal_color = [0.93, 0.55, 0.08, 0.92];
                for placement in proposal_scene
                    .placements
                    .iter()
                    .filter(|placement| self.model.proposal_affected_ids.contains(&placement.id.0))
                {
                    let Some(shelf) = proposal_scene
                        .shelves
                        .iter()
                        .find(|shelf| shelf.id == placement.shelf_id)
                    else {
                        continue;
                    };
                    let (x1, shelf_y) = self.model.world_to_screen(placement.x, shelf.elevation);
                    let (x2, product_top) = self.model.world_to_screen(
                        placement.x + placement.width,
                        shelf.elevation + placement.height,
                    );
                    Self::rect(
                        &mut vertices,
                        w,
                        h,
                        x1 + 2.0,
                        product_top + 2.0,
                        x2 - 2.0,
                        shelf_y - 2.0,
                        [0.95, 0.63, 0.16, 0.24],
                    );
                    Self::dashed_outline(
                        &mut vertices,
                        w,
                        h,
                        x1 - 2.0,
                        product_top - 2.0,
                        x2 + 2.0,
                        shelf_y + 2.0,
                        proposal_color,
                    );
                }
                for placement in self.model.scene.placements.iter().filter(|placement| {
                    self.model.proposal_affected_ids.contains(&placement.id.0)
                        && !proposal_scene
                            .placements
                            .iter()
                            .any(|candidate| candidate.id == placement.id)
                }) {
                    let Some(shelf) = self
                        .model
                        .scene
                        .shelves
                        .iter()
                        .find(|shelf| shelf.id == placement.shelf_id)
                    else {
                        continue;
                    };
                    let (x1, shelf_y) = self.model.world_to_screen(placement.x, shelf.elevation);
                    let (x2, product_top) = self.model.world_to_screen(
                        placement.x + placement.width,
                        shelf.elevation + placement.height,
                    );
                    Self::dashed_outline(
                        &mut vertices,
                        w,
                        h,
                        x1 - 2.0,
                        product_top - 2.0,
                        x2 + 2.0,
                        shelf_y + 2.0,
                        [0.78, 0.20, 0.16, 0.9],
                    );
                }
            }
            if let Some(drag) = &self.model.drag {
                let y = self.model.world_to_screen(Length::ZERO, drag.elevation).1;
                Self::rect(
                    &mut vertices,
                    w,
                    h,
                    (left + right) / 2.0 - 0.75,
                    top_y,
                    (left + right) / 2.0 + 0.75,
                    base_y,
                    [0.94, 0.58, 0.08, 0.9],
                );
                Self::rect(
                    &mut vertices,
                    w,
                    h,
                    right + 12.0,
                    y - 1.0,
                    right + 44.0,
                    y + 1.0,
                    [0.94, 0.58, 0.08, 0.9],
                );
            }
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
    use planogram_core::{CommandResult, DraftVersion, ProductId, VersionId};

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
        let (left, shelf_y) = renderer.world_to_screen(placement.x, shelf.elevation);
        let (right, top) = renderer.world_to_screen(
            placement.x + placement.width,
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
            validation: Default::default(),
        });
        assert!(renderer.scene.placements.is_empty());
        assert_eq!(renderer.selected, None);
    }
}
