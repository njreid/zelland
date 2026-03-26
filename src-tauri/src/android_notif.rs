//! Android-only: shows a rich agent notification with a "Switch to session" action button.
//! Bridges to AgentNotifHelper.kt via JNI using the NDK context initialised by wry/Tauri.

use jni::objects::{JObject, JValue};
use log::warn;

pub fn show(title: &str, body: &str, session_name: &str) {
    let ctx = ndk_context::android_context();

    let vm = match unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) } {
        Ok(v)  => v,
        Err(e) => { warn!("android_notif: no JVM: {e}"); return; }
    };
    let mut env = match vm.attach_current_thread() {
        Ok(e)  => e,
        Err(e) => { warn!("android_notif: attach failed: {e}"); return; }
    };

    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

    let Ok(jtitle)   = env.new_string(title)        else { return; };
    let Ok(jbody)    = env.new_string(body)          else { return; };
    let Ok(jsession) = env.new_string(session_name)  else { return; };

    let Ok(class) = env.find_class("com/njr/zelland/AgentNotifHelper") else {
        warn!("android_notif: AgentNotifHelper class not found");
        return;
    };

    if let Err(e) = env.call_static_method(
        &class,
        "show",
        "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::from(&activity),
            JValue::from(&*jtitle),
            JValue::from(&*jbody),
            JValue::from(&*jsession),
        ],
    ) {
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
        }
        warn!("android_notif: AgentNotifHelper.show failed: {e}");
    }
}
