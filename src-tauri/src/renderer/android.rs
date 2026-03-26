#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject, JString};
#[cfg(target_os = "android")]
use ndk::native_window::NativeWindow;
#[cfg(target_os = "android")]
use raw_window_handle::{AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle};
#[cfg(target_os = "android")]
use log::{info, error};
#[cfg(target_os = "android")]
use tauri::Manager;
#[cfg(target_os = "android")]
use super::{RawWindow, RawDisplay, Renderer};

#[cfg(target_os = "android")]
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

    crate::spawn_on_runtime(async move {
        let is_none = {
            super::RENDERER.lock().unwrap_or_else(|e| e.into_inner()).is_none()
        };

        if is_none {
            Renderer::init().await;
        }

        let mut lock = super::RENDERER.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(renderer) = lock.as_mut() {
            renderer.set_surface(raw_window, raw_display);
            // Do not resize here — passResizeToRust fires immediately after
            // surfaceCreated with the real dimensions.
        }
    });
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_passSurfaceDestroyedToRust(
    _env: JNIEnv,
    _class: JClass,
) {
    info!("passSurfaceDestroyedToRust called from JNI");
    crate::spawn_on_runtime(async move {
        let mut lock = super::RENDERER.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(renderer) = lock.as_mut() {
            renderer.drop_surface();
        }
    });
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_passResizeToRust(
    _env: JNIEnv,
    _class: JClass,
    width: jni::sys::jint,
    height: jni::sys::jint,
) {
    info!("passResizeToRust called from JNI: {}x{}", width, height);
    let width = width as u32;
    let height = height as u32;

    // Always persist the size so set_surface() can pick it up even if
    // the renderer wasn't ready when this JNI call first arrived.
    super::store_pending_size(width, height);

    crate::spawn_on_runtime(async move {
        let mut lock = super::RENDERER.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(renderer) = lock.as_mut() {
            renderer.resize(width, height);
            renderer.render();
        }
    });
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_passTouchToRust(
    mut env: JNIEnv,
    _class: JClass,
    action: JString,
    x: jni::sys::jfloat,
    y: jni::sys::jfloat,
) {
    let action_str: String = env.get_string(&action).expect("Couldn't get java string").into();
    
    if let Some(app) = crate::get_app_handle() {
        let ssh_manager = app.state::<crate::ssh::SshManager>();
        let ssh_manager_inner = ssh_manager.inner().clone();
        
        crate::spawn_on_runtime(async move {
            if let Err(e) = ssh_manager_inner.process_touch(action_str, x as f32, y as f32).await {
                error!("Failed to process touch: {}", e);
            }
        });
    }
}
