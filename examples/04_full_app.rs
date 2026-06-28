// Example: Full application setup
//
// Shows how all pieces fit together in a real Android app that uses multiple bridge classes.
// This mirrors how cedinia would look after migrating from raw JNI to jni-high.
//
// In a real project this would be split across modules (android/file_picker.rs,
// android/notifications.rs, etc.) but is shown here in one file for clarity.

use jni_high::{android_bridge, AndroidContext, BridgeResultExt};

// ---------------------------------------------------------------------------
// Bridge declarations
// ---------------------------------------------------------------------------

android_bridge! {
    // All three classes compile into one DEX file; only one class loader is created.
    dex = include_bytes!(concat!(env!("OUT_DIR"), "/app_bridge.dex")),

    class FilePicker {
        java_name = "com.example.bridge.FilePicker",

        static fn launch_pick_directory(start_path: &str, is_include: bool);
        static fn open_url(url: &str);

        callback fn on_directory_picked(path: String, is_include: bool);
    }

    class Notifications {
        java_name = "com.example.bridge.Notifications",

        static fn send(title: &str, body: &str, id: i32);

        #[no_activity]
        static fn are_enabled() -> bool;
    }

    class Locale {
        java_name = "com.example.bridge.Locale",

        #[no_activity]
        static fn language_tag() -> String;

        // Java method is called "getPreferredHourFormat" but exposed as preferred_hour_format.
        #[java_name = "getPreferredHourFormat"]
        #[no_activity]
        static fn preferred_hour_format() -> i32;
    }
}

// ---------------------------------------------------------------------------
// Initialization - called once from android_main
// ---------------------------------------------------------------------------

pub fn init(app: android_activity::AndroidApp) {
    // Initialize the singleton. Must happen before any bridge call.
    // DEX loading, class lookup, and native registration are deferred to first use.
    AndroidContext::init(app);

    // Register callback handlers upfront.
    // These can be replaced at any time with set_* methods.
    FilePicker::set_on_directory_picked(|path, is_include| {
        if is_include {
            crate::settings::add_include_path(path);
        } else {
            crate::settings::add_exclude_path(path);
        }
    });
}

// ---------------------------------------------------------------------------
// Public API used by the rest of the app
// ---------------------------------------------------------------------------

pub fn open_url(url: &str) {
    FilePicker::open_url(url).log_err("open_url");
}

pub fn pick_directory(start_path: &str, is_include: bool) {
    FilePicker::launch_pick_directory(start_path, is_include)
        .log_err("launch_pick_directory");
}

pub fn notify(title: &str, body: &str, id: i32) {
    if Notifications::are_enabled().unwrap_or(false) {
        Notifications::send(title, body, id).log_err("notify");
    }
}

pub fn language_tag() -> String {
    Locale::language_tag().unwrap_or_else(|_| "en".to_string())
}

pub fn uses_24h_format() -> bool {
    Locale::preferred_hour_format().unwrap_or(12) == 24
}

// ---------------------------------------------------------------------------
// Error handling patterns
// ---------------------------------------------------------------------------

pub fn advanced_error_handling() {
    // Pattern 1: log and discard (fire-and-forget)
    FilePicker::open_url("https://example.com").log_err("open_url");

    // Pattern 2: unwrap with fallback
    let tag = Locale::language_tag().unwrap_or_else(|_| "en".to_string());

    // Pattern 3: propagate with ?
    fn inner() -> jni_high::BridgeResult<()> {
        Notifications::send("Scan done", "Found 42 files", 1)?;
        Ok(())
    }

    // Pattern 4: match on specific errors
    use jni_high::BridgeError;
    match Notifications::are_enabled() {
        Ok(enabled) => log::info!("Notifications enabled: {enabled}"),
        Err(BridgeError::ContextNotInitialized) => log::warn!("init() not called yet"),
        Err(e) => log::error!("Bridge error: {e:?}"),
    }

    let _ = (tag, inner());
}

// ---------------------------------------------------------------------------
// Stubs so this file compiles standalone
// ---------------------------------------------------------------------------

mod settings {
    pub fn add_include_path(_: String) {}
    pub fn add_exclude_path(_: String) {}
}
