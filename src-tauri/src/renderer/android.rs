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
pub extern "system" fn Java_com_njr_zelland_MainActivity_getSelectionText(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    sc: jni::sys::jint,
    sr: jni::sys::jint,
    ec: jni::sys::jint,
    er: jni::sys::jint,
) -> jni::sys::jstring {
    let lock = super::RENDERER.lock().unwrap_or_else(|e| e.into_inner());
    let text = lock.as_ref()
        .map(|r| r.extract_text(sc, sr, ec, er))
        .unwrap_or_default();
    env.new_string(&text)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_setSelectionHighlight(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    sc: jni::sys::jint,
    sr: jni::sys::jint,
    ec: jni::sys::jint,
    er: jni::sys::jint,
    active: jni::sys::jboolean,
) {
    crate::spawn_on_runtime(async move {
        let mut lock = super::RENDERER.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(renderer) = lock.as_mut() {
            renderer.set_selection(sc, sr, ec, er, active != 0);
            renderer.render();
        }
    });
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_passPasteToRust(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    data: jni::objects::JByteArray,
) {
    let bytes: Vec<u8> = env.convert_byte_array(&data)
        .unwrap_or_default()
        .into_iter()
        .map(|b| b as u8)
        .collect();
    if let Some(app) = crate::get_app_handle() {
        let ssh = app.state::<crate::ssh::SshManager>().inner().clone();
        crate::spawn_on_runtime(async move {
            let tab_id = ssh.focused_session.lock().await.clone();
            if let Some(id) = tab_id {
                let _ = ssh.write_input(id, bytes).await;
            }
        });
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_getCellDimensions(
    env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jfloatArray {
    let (w, h) = super::get_cell_size();
    let arr = env.new_float_array(2).expect("new_float_array");
    env.set_float_array_region(&arr, 0, &[w, h]).expect("set_float_array_region");
    arr.into_raw()
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_njr_zelland_MainActivity_updateFontSizeToRust(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    physical_px: jni::sys::jfloat,
) {
    let physical_px = physical_px as f32;
    crate::spawn_on_runtime(async move {
        let mut lock = super::RENDERER.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(renderer) = lock.as_mut() {
            // physical_px is already physical pixels (css_px * dpr computed in Kotlin)
            // update_font_size takes (css_px, dpr) — pass (physical_px, 1.0) to set directly
            renderer.update_font_size(physical_px, 1.0);
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
            if let Err(e) = ssh_manager_inner.process_touch(action_str.clone(), x as f32, y as f32).await {
                log::error!("process_touch failed: {}", e);
            }
        });
    } else {
        log::warn!("passTouchToRust: no app handle yet, dropping touch event");
    }
}
