// Example: Locale and language detection
//
// Shows:
//   - String return value from Java
//   - Option<String> return (nullable Java string)
//   - Multiple bridge classes in one android_bridge! block (same DEX file)
//   - Calling bridges from background threads (attach is thread-safe)
//
// JAVA SIDE: see examples/java/AppLocale.java

use jni_high::{android_bridge, AndroidContext, BridgeResultExt};

// Multiple classes can live in one android_bridge! block if they share a DEX.
// Each class gets its own cached JClass and generated struct.
android_bridge! {
    dex = include_bytes!(concat!(env!("OUT_DIR"), "/locale.dex")),

    class Locale {
        java_name = "com.example.AppLocale",

        // Returns the BCP 47 language tag, e.g. "pl-PL" or "en-US".
        // #[no_activity] because Locale.getDefault() needs no Context.
        #[no_activity]
        static fn language_tag() -> String;

        // Returns None if the system locale is unavailable (very rare).
        #[no_activity]
        static fn country_code() -> Option<String>;

        // Returns 24 or 12 depending on the user's time format preference.
        #[no_activity]
        static fn preferred_hour_format() -> i32;
    }

    class DeviceInfo {
        java_name = "com.example.AppDeviceInfo",

        // Returns Android SDK version as an int (e.g. 34 for Android 14).
        #[no_activity]
        static fn sdk_version() -> i32;

        // Returns the device model string (e.g. "Pixel 8").
        #[no_activity]
        static fn model() -> String;
    }
}

// --- Initialization ----------------------------------------------------------

pub fn init(app: android_activity::AndroidApp) {
    AndroidContext::init(app);
}

// --- Call sites --------------------------------------------------------------

pub fn get_language_tag() -> String {
    Locale::language_tag().unwrap_or_else(|_| "en".to_string())
}

pub fn get_country() -> Option<String> {
    // BridgeResult<Option<String>> - unwrap the bridge error, keep the Option.
    Locale::country_code().unwrap_or(None)
}

pub fn uses_24h_format() -> bool {
    Locale::preferred_hour_format().unwrap_or(12) == 24
}

pub fn android_sdk() -> i32 {
    DeviceInfo::sdk_version().unwrap_or(0)
}

pub fn device_model() -> String {
    DeviceInfo::model().unwrap_or_else(|_| "Unknown".to_string())
}

// --- Background thread usage -------------------------------------------------
//
// Android JNI requires attaching the current thread to the JVM before any call.
// jni-high handles this transparently - attach() wraps each bridge call.
// Calling from a rayon thread pool, a tokio task, or a std::thread all work.

pub fn detect_locale_in_background() {
    std::thread::spawn(|| {
        // No special setup needed - attach is automatic.
        let tag = get_language_tag();
        let sdk = android_sdk();
        log::info!("Running on Android SDK {sdk}, locale {tag}");
    });
}
