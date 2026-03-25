use std::sync::{Arc, Mutex};
use wgpu;
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, DisplayHandle, HandleError,
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
};
use ndk::native_window::NativeWindow;
use jni::JNIEnv;
use jni::objects::{JClass, JObject};
use log::{info, error};
use once_cell::sync::Lazy;
use glyphon::{
    FontSystem, SwashCache, TextAtlas, TextRenderer, Viewport,
    Resolution, TextArea, TextBounds, Color, Metrics,
};

pub const CELL_WIDTH: f32 = 24.0;
pub const CELL_HEIGHT: f32 = 32.0;

pub struct Renderer {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    
    // Glyphon / Text rendering
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    viewport: Viewport,
    text_buffer: glyphon::Buffer,
}

static RENDERER: Lazy<Mutex<Option<Renderer>>> = Lazy::new(|| Mutex::new(None));

impl Renderer {
    pub async fn init() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Zelland Renderer Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        // Initialize Glyphon
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache_format = wgpu::TextureFormat::Rgba8UnormSrgb; // Common format
        let atlas = TextAtlas::new(&device, &queue, &swash_cache, cache_format);
        let text_renderer = TextRenderer::new(&atlas, &device, wgpu::MultisampleState::default(), None);
        let viewport = Viewport::new(&device, &cache_format);
        let text_buffer = glyphon::Buffer::new(&mut font_system, Metrics::new(CELL_HEIGHT * 0.75, CELL_HEIGHT));

        let mut renderer = RENDERER.lock().unwrap();
        *renderer = Some(Renderer {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            config: None,
            font_system,
            swash_cache,
            atlas,
            text_renderer,
            viewport,
            text_buffer,
        });
        
        info!("Renderer initialized with Glyphon");
    }

    pub fn set_surface(&mut self, window: RawWindow, display: RawDisplay) {
        let surface = unsafe {
            self.instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_window_handle: window.window_handle().unwrap().as_raw(),
                    raw_display_handle: display.display_handle().unwrap().as_raw(),
                }
            ).expect("Failed to create wgpu surface")
        };

        // Configure surface for the first time
        let caps = surface.get_capabilities(&self.adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: 1, // Will be resized
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.config = Some(config);
        
        // Re-initialize viewport with correct format
        self.viewport = Viewport::new(&self.device, &caps.formats[0]);
        
        info!("wgpu surface and viewport configured");
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let (Some(surface), Some(config)) = (self.surface.as_mut(), self.config.as_mut()) {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
            self.viewport.update(&self.queue, Resolution { width, height });
            info!("Renderer resized to {}x{}", width, height);
        }
    }

    pub fn render(&mut self) {
        let (surface, config) = match (self.surface.as_ref(), self.config.as_ref()) {
            (Some(s), Some(c)) => (s, c),
            _ => return,
        };

        let frame = surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.05,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render text
            self.text_renderer.render(
                &self.atlas,
                &self.viewport,
                &mut _rpass,
            ).expect("Failed to render text");
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        self.atlas.trim();
    }

    pub fn draw_terminal_grid(&mut self, text: &str) {
        let width = self.config.as_ref().map(|c| c.width as f32).unwrap_or(800.0);
        let height = self.config.as_ref().map(|c| c.height as f32).unwrap_or(600.0);
        
        self.text_buffer.set_size(&mut self.font_system, Some(width), Some(height));
        self.text_buffer.set_text(&mut self.font_system, text, glyphon::Attrs::new().family(glyphon::Family::Monospace), glyphon::Shaping::Advanced);
        self.text_buffer.shape_until_scroll(&mut self.font_system, false);

        self.text_renderer.prepare(
            &self.device,
            &self.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            [TextArea {
                buffer: &self.text_buffer,
                left: 10.0,
                top: 10.0,
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
        ).expect("Failed to prepare text for rendering");
    }

    pub fn draw_ghostty_state(&mut self, state: &mut GhosttyRenderStateWrapper) {
        let dirty = state.get_dirty();
        if dirty == crate::ghostty::GhosttyRenderStateDirty_GHOSTTY_RENDER_STATE_DIRTY_FALSE {
            return;
        }

        let width = self.config.as_ref().map(|c| c.width as f32).unwrap_or(800.0);
        let height = self.config.as_ref().map(|c| c.height as f32).unwrap_or(600.0);
        
        self.text_buffer.set_size(&mut self.font_system, Some(width), Some(height));
        
        let mut full_text = String::with_capacity(8192);
        state.with_rows(|_line, cells| {
            // In a more advanced implementation, we could use state.is_row_dirty()
            // and Glyphon's partial buffer updates, but for now we rebuild if any part is dirty.
            while unsafe { crate::ghostty::ghostty_render_state_row_cells_next(*cells) } {
                let graphemes = crate::ghostty::get_cell_graphemes(*cells);
                if graphemes.is_empty() {
                    full_text.push(' ');
                } else {
                    for &cp in &graphemes {
                        if let Some(c) = std::char::from_u32(cp) {
                            full_text.push(c);
                        }
                    }
                }
            }
            full_text.push('\n');
        });

        self.text_buffer.set_text(&mut self.font_system, &full_text, glyphon::Attrs::new().family(glyphon::Family::Monospace), glyphon::Shaping::Advanced);
        self.text_buffer.shape_until_scroll(&mut self.font_system, false);

        self.text_renderer.prepare(
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
        ).expect("Failed to prepare Ghostty state for rendering");
    }
}

pub fn with_renderer<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Renderer) -> R,
{
    let mut lock = RENDERER.lock().unwrap();
    lock.as_mut().map(f)
}

pub struct RawWindow {
    handle: RawWindowHandle,
}

unsafe impl Send for RawWindow { }

impl HasWindowHandle for RawWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        unsafe { Ok(WindowHandle::borrow_raw(self.handle)) }
    }
}

pub struct RawDisplay {
    handle: RawDisplayHandle,
}

unsafe impl Send for RawDisplay { }

impl HasDisplayHandle for RawDisplay {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        unsafe { Ok(DisplayHandle::borrow_raw(self.handle)) }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_passSurfaceToRust(
    env: JNIEnv,
    _class: JClass,
    surface: JObject,
) {
    info!("passSurfaceToRust called from JNI");
    
    let native_window = unsafe {
        let ptr = ndk_sys::ANativeWindow_fromSurface(env.get_native_interface(), surface.as_raw());
        if ptr.is_null() {
            error!("Failed to get ANativeWindow from surface");
            return;
        }
        NativeWindow::from_ptr(std::ptr::NonNull::new(ptr).unwrap())
    };

    let window_ptr = native_window.ptr().as_ptr() as *mut std::ffi::c_void;

    let window_handle = AndroidNdkWindowHandle::new(std::ptr::NonNull::new(window_ptr).unwrap());
    let handle = RawWindowHandle::AndroidNdk(window_handle);
    
    let display_handle = RawDisplayHandle::Android(AndroidDisplayHandle::new());

    let raw_window = RawWindow { handle };
    let raw_display = RawDisplay { handle: display_handle };

    tokio::spawn(async move {
        let is_none = {
            RENDERER.lock().unwrap().is_none()
        };

        if is_none {
            Renderer::init().await;
        }

        let mut lock = RENDERER.lock().unwrap();
        if let Some(renderer) = lock.as_mut() {
            renderer.set_surface(raw_window, raw_display);
            
            // Trigger a first render to verify Phase 2
            renderer.resize(1080, 2400); // Placeholder for a Pixel 6-9 ish screen
            renderer.draw_terminal_grid("Hello from Zelland Native Surface (Ghostty/wgpu)!");
            renderer.render();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_passTouchToRust(
    mut env: JNIEnv,
    _class: JClass,
    action: jni::objects::JString,
    x: jni::sys::jfloat,
    y: jni::sys::jfloat,
) {
    let action_str: String = env.get_string(&action).expect("Couldn't get java string").into();
    
    if let Some(app) = crate::get_app_handle() {
        let ssh_manager = app.state::<crate::ssh::SshManager>();
        let ssh_manager_clone = ssh_manager.inner().clone();
        
        tokio::spawn(async move {
            if let Err(e) = ssh_manager_clone.process_touch(action_str, x as f32, y as f32).await {
                error!("Failed to process touch: {}", e);
            }
        });
    }
}
