use std::sync::Mutex;
use wgpu;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use log::{info, error};
use once_cell::sync::Lazy;
use glyphon::{
    Cache, FontSystem, SwashCache, TextAtlas, TextRenderer, Viewport,
    Resolution, TextArea, TextBounds, Color, Metrics, Weight, Style,
};
use crate::ghostty::GhosttyRenderStateWrapper;

pub const CELL_WIDTH: f32 = 17.0;
pub const CELL_HEIGHT: f32 = 38.0;

/// WGSL shader for drawing the solid cursor rectangle.
const CURSOR_SHADER: &str = r#"
struct CursorUniforms {
    rect:  vec4<f32>,   // x_left, y_top, x_right, y_bottom — all in NDC
    color: vec4<f32>,   // r, g, b, a
}
@group(0) @binding(0) var<uniform> u: CursorUniforms;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x0 = u.rect.x; let x1 = u.rect.z;
    let y0 = u.rect.w; let y1 = u.rect.y;
    var xs = array<f32,6>(x0, x1, x0, x1, x1, x0);
    var ys = array<f32,6>(y1, y1, y0, y1, y0, y0);
    return vec4<f32>(xs[i], ys[i], 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return u.color;
}
"#;

/// A single styled run of text within one terminal row.
#[derive(Clone, PartialEq)]
struct CellRun {
    text: String,
    /// Foreground colour as (r, g, b).
    fg: (u8, u8, u8),
    bold: bool,
    italic: bool,
}

pub struct Renderer {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    pending_size: Option<(u32, u32)>,

    // Glyphon / Text rendering
    _cache: Cache,
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    atlas_format: wgpu::TextureFormat,
    text_renderer: TextRenderer,
    viewport: Viewport,
    text_buffer: glyphon::Buffer,

    // Per-row damage cache: each row is a list of styled runs.
    row_cache: Vec<Vec<CellRun>>,
    // Reusable buffer for span tuples passed to set_rich_text; avoids a
    // Vec allocation + String clones on frames where text has changed.
    span_buf: Vec<(String, Weight, Style, Color)>,

    // Current cell dimensions in physical pixels (updated via set_terminal_font_size).
    cell_width: f32,
    cell_height: f32,

    // Cursor rectangle rendering.
    cursor_pipeline: Option<wgpu::RenderPipeline>,
    cursor_bind_group_layout: Option<wgpu::BindGroupLayout>,
    cursor_uniform_buf: Option<wgpu::Buffer>,
    cursor_bind_group: Option<wgpu::BindGroup>,
    /// Cursor position as (pixel_x, pixel_y, cell_w, cell_h); None if hidden.
    cursor_pixel_rect: Option<(f32, f32, f32, f32)>,

    debug_frames_saved: u32,
}

static RENDERER: Lazy<Mutex<Option<Renderer>>> = Lazy::new(|| Mutex::new(None));

/// Surface size stored independently of the Renderer so it can be written by
/// `passResizeToRust` even before the renderer finishes its ~200 ms wgpu init.
/// `set_surface` reads this as a fallback when `pending_size` is None.
static PENDING_SIZE: Lazy<Mutex<Option<(u32, u32)>>> = Lazy::new(|| Mutex::new(None));

pub fn store_pending_size(width: u32, height: u32) {
    if let Ok(mut g) = PENDING_SIZE.lock() {
        *g = Some((width, height));
    }
}

pub fn update_font_size_global(css_px: f32, dpr: f32) {
    crate::spawn_on_runtime(async move {
        let mut lock = RENDERER.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(renderer) = lock.as_mut() {
            renderer.update_font_size(css_px, dpr);
        }
    });
}

impl Renderer {
    pub async fn init() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
        {
            Some(a) => a,
            None => {
                error!("Failed to find a suitable GPU adapter");
                return;
            }
        };

        let adapter_limits = adapter.limits();
        info!("adapter max_texture_dimension_2d: {}", adapter_limits.max_texture_dimension_2d);

        let (device, queue) = match adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Zelland Renderer Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter_limits),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
        {
            Ok(dq) => dq,
            Err(e) => {
                error!("Failed to create wgpu device: {}", e);
                return;
            }
        };
        info!("device max_texture_dimension_2d: {}", device.limits().max_texture_dimension_2d);

        let mut font_system = FontSystem::new();

        // Load the bundled Noto Sans Mono Nerd Font (copied from assets on startup).
        // This is tried first so Nerd Font glyphs render correctly.
        let nerd_font_dir = "/data/data/com.njr.zelland/files/fonts";
        for name in &[
            "NotoSansMNerdFontMono-Regular.ttf",
            "NotoSansMNerdFontMono-Bold.ttf",
        ] {
            let path = format!("{}/{}", nerd_font_dir, name);
            match font_system.db_mut().load_font_file(&path) {
                Ok(()) => info!("Loaded bundled font: {}", name),
                Err(e) => error!("Failed to load bundled font {}: {}", name, e),
            }
        }

        // On Android, fontdb's system font discovery may be blocked by SELinux.
        // Fall back to well-known system paths if the db is still empty.
        if font_system.db().faces().count() == 0 {
            info!("Font database empty — trying Android system font paths");
            for path in &[
                "/system/fonts/NotoSansMono-Regular.ttf",
                "/system/fonts/DroidSansMono.ttf",
                "/system/fonts/NotoMono-Regular.ttf",
                "/system/fonts/Roboto-Regular.ttf",
                "/system/fonts/NotoSans-Regular.ttf",
                "/system/fonts/DroidSans.ttf",
            ] {
                if let Ok(()) = font_system.db_mut().load_font_file(path) {
                    info!("Loaded system font from {}", path);
                }
            }
        }
        let count = font_system.db().faces().count();
        if count == 0 {
            error!("No fonts loaded — text rendering will panic");
        } else {
            info!("Font database has {} face(s)", count);
        }

        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        // Use a placeholder format; will be rebuilt to match the actual surface
        // format in set_surface() to avoid a pipeline/attachment format mismatch.
        let cache_format = wgpu::TextureFormat::Bgra8Unorm;
        let mut atlas = TextAtlas::new(&device, &queue, &cache, cache_format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let viewport = Viewport::new(&device, &cache);
        let text_buffer = glyphon::Buffer::new(
            &mut font_system,
            Metrics::new(CELL_HEIGHT * 0.75, CELL_HEIGHT),
        );

        let mut renderer = RENDERER.lock().unwrap();
        *renderer = Some(Renderer {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            config: None,
            pending_size: None,
            _cache: cache,
            font_system,
            swash_cache,
            atlas,
            atlas_format: cache_format,
            text_renderer,
            viewport,
            text_buffer,
            row_cache: Vec::new(),
            span_buf: Vec::new(),
            cell_width: CELL_WIDTH,
            cell_height: CELL_HEIGHT,
            cursor_pipeline: None,
            cursor_bind_group_layout: None,
            cursor_uniform_buf: None,
            cursor_bind_group: None,
            cursor_pixel_rect: None,
            debug_frames_saved: 0,
        });

        // Build the initial cursor pipeline using the existing guard — no second lock needed.
        if let Some(r) = renderer.as_mut() {
            r.build_cursor_resources();
        }

        info!("Renderer initialized");
    }

    pub fn set_surface(&mut self, window: RawWindow, display: RawDisplay) {
        let surface = unsafe {
            match self.instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_window_handle: window.window_handle().unwrap().as_raw(),
                    raw_display_handle: display.display_handle().unwrap().as_raw(),
                },
            ) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create wgpu surface: {}", e);
                    return;
                }
            }
        };

        let caps = surface.get_capabilities(&self.adapter);
        let surface_format = caps.formats[0];
        info!("Surface format: {:?}, available: {:?}", surface_format, caps.formats);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.config = Some(config);

        // Rebuild the glyphon atlas/pipeline if the surface format changed.
        // The atlas format must match the render pass attachment format or text
        // will silently not appear (pipeline/attachment format mismatch).
        if surface_format != self.atlas_format {
            info!("Atlas format {:?} != surface format {:?} — rebuilding text pipeline",
                self.atlas_format, surface_format);
            self.rebuild_text_pipeline(surface_format);
        }

        info!("wgpu surface configured");

        // Apply any resize that arrived before the surface was ready.
        // Check both the per-Renderer field and the global static — the global
        // is written by passResizeToRust even before the Renderer is initialised,
        // so it catches the common race where wgpu init takes ~200 ms.
        let pending = self.pending_size.take().or_else(|| {
            PENDING_SIZE.lock().ok().and_then(|mut g| g.take())
        });
        if let Some((w, h)) = pending {
            info!("Applying pending resize {}x{} after surface created", w, h);
            self.resize(w, h);
            self.render();
        } else {
            info!("set_surface: no pending resize — surface stays at 1x1 until passResizeToRust fires");
        }
    }

    /// Rebuild the glyphon text atlas and renderer pipeline for a new texture format.
    /// Must be called whenever the surface format changes to avoid a pipeline mismatch
    /// that silently prevents text from rendering.
    fn rebuild_text_pipeline(&mut self, format: wgpu::TextureFormat) {
        let mut atlas = TextAtlas::new(&self.device, &self.queue, &self._cache, format);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            &self.device,
            wgpu::MultisampleState::default(),
            None,
        );
        self.atlas = atlas;
        self.atlas_format = format;
        self.text_renderer = text_renderer;
        self.row_cache.clear();
        info!("Text pipeline rebuilt for format {:?}", format);
        self.build_cursor_resources();
    }

    /// Create (or recreate) the cursor shader pipeline and its uniform resources.
    /// Called once on init and again whenever the surface format changes.
    fn build_cursor_resources(&mut self) {
        let format = self.config
            .as_ref()
            .map(|c| c.format)
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cursor_shader"),
            source: wgpu::ShaderSource::Wgsl(CURSOR_SHADER.into()),
        });

        let bgl = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cursor_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = self.device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("cursor_layout"),
                bind_group_layouts: &[&bgl],
                push_constant_ranges: &[],
            },
        );

        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cursor_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // 32-byte uniform buffer: vec4 rect + vec4 color.
        let uniform_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cursor_uniform_buf"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cursor_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        self.cursor_pipeline = Some(pipeline);
        self.cursor_bind_group_layout = Some(bgl);
        self.cursor_uniform_buf = Some(uniform_buf);
        self.cursor_bind_group = Some(bind_group);
    }

    /// Write NDC cursor rect + white color into the uniform buffer.
    fn update_cursor_uniforms(&self, px: f32, py: f32, pw: f32, ph: f32) {
        let (sw, sh) = match self.config.as_ref() {
            Some(c) => (c.width as f32, c.height as f32),
            None => return,
        };
        let x0 =  (px        / sw) * 2.0 - 1.0;
        let x1 =  ((px + pw) / sw) * 2.0 - 1.0;
        let y1 = 1.0 -  (py        / sh) * 2.0;   // top in NDC (larger y)
        let y0 = 1.0 - ((py + ph) / sh) * 2.0;    // bottom in NDC (smaller y)
        // Uniform layout: rect(x_left, y_top, x_right, y_bottom), color(r,g,b,a)
        let data: [f32; 8] = [x0, y1, x1, y0,  1.0, 1.0, 1.0, 1.0];
        let bytes = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, 32)
        };
        if let Some(buf) = &self.cursor_uniform_buf {
            self.queue.write_buffer(buf, 0, bytes);
        }
    }

    /// Release the wgpu surface. Called when the Android SurfaceView is
    /// destroyed (e.g. screen lock). The next `set_surface` call re-attaches.
    pub fn drop_surface(&mut self) {
        self.surface = None;
        self.config = None;
        info!("wgpu surface released");
    }

    /// Update the font size used for both rendering and cell dimension reporting.
    /// css_px is the font size in CSS/logical pixels; dpr is window.devicePixelRatio.
    pub fn update_font_size(&mut self, css_px: f32, dpr: f32) {
        let physical = css_px * dpr;
        self.cell_height = physical;
        self.cell_width = physical * 0.45;
        let font_size = physical * 0.75;
        self.text_buffer.set_metrics(
            &mut self.font_system,
            Metrics::new(font_size, physical),
        );
        self.row_cache.clear();
        info!("Font size updated: css={}px dpr={} cell={}×{} font={}px",
              css_px, dpr, self.cell_width, self.cell_height, font_size);
        self.render();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let max_dim = self.device.limits().max_texture_dimension_2d;
        let width = width.min(max_dim).max(1);
        let height = height.min(max_dim).max(1);
        info!("resize: {}x{} (max_dim={})", width, height, max_dim);
        if self.surface.is_none() {
            self.pending_size = Some((width, height));
            info!("resize {}x{} deferred (no surface yet)", width, height);
            return;
        }
        if let (Some(surface), Some(config)) = (self.surface.as_mut(), self.config.as_mut()) {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
            self.viewport.update(&self.queue, Resolution { width, height });
            // Invalidate the row cache so the first post-resize frame is a full redraw.
            self.row_cache.clear();
            info!("Renderer resized to {}x{}", width, height);
        }
    }

    pub fn render(&mut self) {
        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return,
        };

        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to acquire swap chain texture: {}", e);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // Write cursor uniform data before opening any render passes.
        if let Some((cx, cy, cw, ch)) = self.cursor_pixel_rect {
            self.update_cursor_uniforms(cx, cy, cw, ch);
        }

        // Pass 1: clear + cursor rectangle.
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_cursor_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if self.cursor_pixel_rect.is_some() {
                if let (Some(pipeline), Some(bg)) =
                    (&self.cursor_pipeline, &self.cursor_bind_group)
                {
                    rpass.set_pipeline(pipeline);
                    rpass.set_bind_group(0, bg, &[]);
                    rpass.draw(0..6, 0..1);
                }
            }
        }

        // Pass 2: text on top of cursor.
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Err(e) = self.text_renderer.render(&self.atlas, &self.viewport, &mut rpass) {
                error!("Failed to render text: {}", e);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        self.atlas.trim();

        if self.debug_frames_saved < 3 {
            self.debug_frames_saved += 1;
            self.save_debug_frame();
        }
    }

    fn save_debug_frame(&mut self) {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return,
        };
        let width = config.width;
        let height = config.height;
        let format = config.format;

        // Align bytes_per_row to 256 as required by wgpu.
        let bytes_per_pixel: u32 = 4;
        let unpadded_row = width * bytes_per_pixel;
        let align: u32 = 256;
        let padded_row = (unpadded_row + align - 1) / align * align;

        let capture_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let capture_view = capture_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &capture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.8, g: 0.0, b: 0.8, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let _ = self.text_renderer.render(&self.atlas, &self.viewport, &mut rpass);
        }

        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture_buf"),
            size: (padded_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &capture_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buf,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(enc.finish()));

        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::Maintain::Wait);

        let data = slice.get_mapped_range();
        // Write PPM (P6 binary): each row strip padded_row bytes, we take unpadded_row.
        let path = format!("/sdcard/Android/data/com.njr.zelland/files/frame_{}.ppm", self.debug_frames_saved);
        let mut out = format!("P6\n{} {}\n255\n", width, height).into_bytes();
        for row in 0..height as usize {
            let start = row * padded_row as usize;
            let row_data = &data[start..start + unpadded_row as usize];
            // wgpu format is Bgra or Rgba — convert to RGB for PPM.
            for chunk in row_data.chunks(4) {
                // Assume Bgra8: swap B and R for PPM RGB.
                out.push(chunk[2]); // R
                out.push(chunk[1]); // G
                out.push(chunk[0]); // B
            }
        }
        drop(data);
        buf.unmap();

        match std::fs::write(&path, &out) {
            Ok(_) => info!("Debug frame saved to {}", path),
            Err(e) => error!("Failed to save debug frame: {}", e),
        }
    }

    pub fn draw_ghostty_state(&mut self, state: &mut GhosttyRenderStateWrapper, cursor_pos: (u16, u16)) {
        let dirty = state.get_dirty();
        if dirty == crate::ghostty::GhosttyRenderStateDirty_GHOSTTY_RENDER_STATE_DIRTY_FALSE {
            return;
        }

        let width = self.config.as_ref().map(|c| c.width as f32).unwrap_or(800.0);
        let height = self.config.as_ref().map(|c| c.height as f32).unwrap_or(600.0);

        self.text_buffer
            .set_size(&mut self.font_system, Some(width), Some(height));

        let mut changed = false;
        state.with_rows(|line_idx, is_dirty, cells| {
            if is_dirty || line_idx as usize >= self.row_cache.len() {
                let runs = build_row_runs(cells);

                let row_idx = line_idx as usize;
                if row_idx >= self.row_cache.len() {
                    self.row_cache.resize(row_idx + 1, Vec::new());
                }
                if self.row_cache[row_idx] != runs {
                    self.row_cache[row_idx] = runs;
                    changed = true;
                }
            }
        });

        // Shrink cache when the terminal gets smaller (prevents ghost rows).
        let (_num_cols, num_rows) = state.get_size();
        if self.row_cache.len() > num_rows as usize {
            self.row_cache.truncate(num_rows as usize);
            changed = true;
        }

        // Update cursor position every frame regardless of text change.
        let (cur_col, cur_row) = cursor_pos;
        self.cursor_pixel_rect = Some((
            cur_col as f32 * self.cell_width,
            cur_row as f32 * self.cell_height,
            self.cell_width,
            self.cell_height,
        ));

        // Nothing in the text buffer changed — skip the expensive text
        // pipeline update entirely (set_rich_text + shape + prepare).
        if !changed {
            return;
        }

        // Build (text, attrs) spans for set_rich_text, inserting newlines
        // between rows so glyphon lays each row on its own line.
        // Reuse the pre-allocated buffer to avoid a Vec alloc per frame.
        self.span_buf.clear();
        let row_count = self.row_cache.len();
        for (row_idx, row) in self.row_cache.iter().enumerate() {
            for run in row {
                let (r, g, b) = run.fg;
                self.span_buf.push((
                    run.text.clone(),
                    if run.bold { Weight::BOLD } else { Weight::NORMAL },
                    if run.italic { Style::Italic } else { Style::Normal },
                    Color::rgb(r, g, b),
                ));
            }
            if row_idx + 1 < row_count {
                self.span_buf.push((
                    "\n".to_string(),
                    Weight::NORMAL,
                    Style::Normal,
                    Color::rgb(255, 255, 255),
                ));
            }
        }

        // Scope the split borrows so they're dropped before `prepare` needs
        // an immutable reference to `self.text_buffer`.
        {
            let text_buffer = &mut self.text_buffer;
            let font_system = &mut self.font_system;
            text_buffer.set_rich_text(
                font_system,
                self.span_buf.iter().map(|(text, weight, style, color)| {
                    let mut attrs = glyphon::Attrs::new()
                        .family(glyphon::Family::Monospace)
                        .weight(*weight)
                        .style(*style);
                    attrs.color_opt = Some(*color);
                    (text.as_str(), attrs)
                }),
                glyphon::Attrs::new().family(glyphon::Family::Monospace),
                // Basic shaping is sufficient for monospace terminal text and
                // avoids the ligature/complex-script overhead of Advanced.
                glyphon::Shaping::Basic,
            );
            text_buffer.shape_until_scroll(font_system, false);
        }

        if let Err(e) = self.text_renderer.prepare(
            &self.device,
            &self.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            [TextArea {
                buffer: &self.text_buffer,
                left: 0.0,
                top: 0.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                },
                default_color: Color::rgb(255, 255, 255),
                custom_glyphs: &[],
            }],
            &mut self.swash_cache,
        ) {
            error!("Failed to prepare Ghostty state for rendering: {}", e);
        }
    }
}

/// Build a list of styled runs for a single terminal row.
fn build_row_runs(cells: &crate::ghostty::GhosttyRenderStateRowCells) -> Vec<CellRun> {
    let mut runs: Vec<CellRun> = Vec::new();
    let mut cur_text = String::with_capacity(128);
    let mut cur_fg: (u8, u8, u8) = (255, 255, 255);
    let mut cur_bold = false;
    let mut cur_italic = false;
    let mut first = true;

    while unsafe { crate::ghostty::ghostty_render_state_row_cells_next(*cells) } {
        let style = crate::ghostty::get_cell_style(*cells);
        let graphemes = crate::ghostty::get_cell_graphemes(*cells);

        let cell_text: String = if graphemes.is_empty() {
            " ".to_string()
        } else {
            graphemes
                .iter()
                .filter_map(|&cp| std::char::from_u32(cp))
                .collect()
        };

        let fg = if style.inverse {
            ghostty_color_to_rgb(&style.bg_color).unwrap_or((0, 0, 0))
        } else {
            ghostty_color_to_rgb(&style.fg_color).unwrap_or((255, 255, 255))
        };
        let bold = style.bold;
        let italic = style.italic;

        if first {
            cur_fg = fg;
            cur_bold = bold;
            cur_italic = italic;
            cur_text.push_str(&cell_text);
            first = false;
        } else if fg == cur_fg && bold == cur_bold && italic == cur_italic {
            cur_text.push_str(&cell_text);
        } else {
            if !cur_text.is_empty() {
                runs.push(CellRun {
                    text: cur_text.clone(),
                    fg: cur_fg,
                    bold: cur_bold,
                    italic: cur_italic,
                });
            }
            cur_text.clear();
            cur_text.push_str(&cell_text);
            cur_fg = fg;
            cur_bold = bold;
            cur_italic = italic;
        }
    }
    if !cur_text.is_empty() {
        runs.push(CellRun {
            text: cur_text,
            fg: cur_fg,
            bold: cur_bold,
            italic: cur_italic,
        });
    }
    runs
}

/// Convert a Ghostty style colour to an RGB triple, or `None` for the
/// terminal default (caller should substitute white for fg, black for bg).
#[allow(non_upper_case_globals)]
fn ghostty_color_to_rgb(
    color: &crate::ghostty::GhosttyStyleColor,
) -> Option<(u8, u8, u8)> {
    use crate::ghostty::*;
    match color.tag {
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_NONE => None,
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_PALETTE => {
            Some(ansi_palette_color(unsafe { color.value.palette } as usize))
        }
        GhosttyStyleColorTag_GHOSTTY_STYLE_COLOR_RGB => {
            let rgb = unsafe { color.value.rgb };
            Some((rgb.r, rgb.g, rgb.b))
        }
        _ => None,
    }
}

/// Map a 256-colour palette index to an RGB triple using standard xterm colours.
fn ansi_palette_color(idx: usize) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (170, 0, 0),
        (0, 170, 0),
        (170, 170, 0),
        (0, 0, 170),
        (170, 0, 170),
        (0, 170, 170),
        (170, 170, 170),
        (85, 85, 85),
        (255, 85, 85),
        (85, 255, 85),
        (255, 255, 85),
        (85, 85, 255),
        (255, 85, 255),
        (85, 255, 255),
        (255, 255, 255),
    ];
    if idx < 16 {
        return PALETTE[idx];
    }
    if idx < 232 {
        // 6×6×6 colour cube (indices 16–231)
        let i = idx - 16;
        let to_level = |v: usize| if v == 0 { 0u8 } else { (55 + v * 40) as u8 };
        return (to_level(i / 36), to_level((i / 6) % 6), to_level(i % 6));
    }
    // Greyscale ramp (indices 232–255)
    let v = (8 + (idx - 232) * 10) as u8;
    (v, v, v)
}

pub fn with_renderer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Renderer) -> R,
{
    let mut lock = RENDERER.lock().unwrap_or_else(|e| e.into_inner());
    lock.as_mut().map(f)
}

/// Returns the current cell dimensions in physical pixels.
/// Falls back to the compile-time constants if no renderer exists yet.
pub fn get_cell_size() -> (f32, f32) {
    with_renderer(|r| (r.cell_width, r.cell_height))
        .unwrap_or((CELL_WIDTH, CELL_HEIGHT))
}

pub struct RawWindow {
    handle: RawWindowHandle,
}

unsafe impl Send for RawWindow {}

impl HasWindowHandle for RawWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        unsafe { Ok(WindowHandle::borrow_raw(self.handle)) }
    }
}

pub struct RawDisplay {
    handle: RawDisplayHandle,
}

unsafe impl Send for RawDisplay {}

impl HasDisplayHandle for RawDisplay {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        unsafe { Ok(DisplayHandle::borrow_raw(self.handle)) }
    }
}

pub mod android;
