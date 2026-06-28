// Example: Notifications bridge
//
// Shows:
//   - #[no_activity]  -> Java method takes no Activity parameter
//   - bool return     -> Rust gets a typed bool back from Java
//   - Multiple methods in one class
//   - Calling from anywhere (no &self, no stored reference needed)
//
// JAVA SIDE: see examples/java/AppNotifications.java

use jni_high::{android_bridge, AndroidContext, BridgeResultExt};

android_bridge! {
    dex = include_bytes!(concat!(env!("OUT_DIR"), "/notifications.dex")),

    class Notifications {
        java_name = "com.example.AppNotifications",

        // Checks if the user granted notification permission.
        // No Activity needed - pure query against the NotificationManager.
        #[no_activity]
        static fn are_enabled() -> bool;

        // Opens Android's system notification settings screen.
        static fn open_settings();

        // Posts a scan-completed notification.
        // Java handles the NotificationCompat.Builder boilerplate.
        static fn send_scan_completed(file_count: i32, in_background: bool);
    }
}

// --- Initialization ----------------------------------------------------------

pub fn init(app: android_activity::AndroidApp) {
    AndroidContext::init(app);
    // No callbacks in this bridge - no set_* calls needed.
}

// --- Call sites --------------------------------------------------------------

pub fn are_notifications_enabled() -> bool {
    // #[no_activity] methods don't inject the Activity, so they can be called
    // from a background thread without an active window.
    Notifications::are_enabled().unwrap_or(false)
}

pub fn open_notification_settings() {
    Notifications::open_settings().log_err("open_notification_settings");
}

pub fn notify_scan_done(count: usize, background: bool) {
    // i32 because Java int is i32; usize doesn't cross the JNI boundary directly.
    Notifications::send_scan_completed(count as i32, background)
        .log_err("notify_scan_done");
}

// ---------------------------------------------------------------------------
// BEFORE - raw JNI equivalent of are_notifications_enabled() alone:
// ---------------------------------------------------------------------------
//
// pub fn are_system_notifications_enabled() -> bool {
//     let Some(app) = get_android_app() else { return false };
//     let Some(activity_ref) = get_activity_global_ref() else { return false };
//     let Some(vm) = try_jvm(&app) else { return false };
//
//     let result = vm.attach_current_thread(|env| {
//         use jni::objects::{JObject, JValue};
//
//         let svc_name = env.new_string("notification")?;
//         let nm = env.call_method(
//             &*activity_ref,
//             jni_str!("getSystemService"),
//             jni_sig!((name: java.lang.String) -> java.lang.Object),
//             &[JValue::Object(&svc_name)],
//         )?.l()?;
//
//         Ok(env.call_method(
//             &nm,
//             jni_str!("areNotificationsEnabled"),
//             jni_sig!(() -> boolean),
//             &[],
//         )?.z()?)
//     });
//     result.unwrap_or(false)
// }
//
// AFTER - one line per call site. Java encapsulates the boilerplate.
