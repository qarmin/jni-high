// Example: File picker bridge
//
// Shows the most common jni-high pattern:
//   - static fn  -> Rust calls a Java static method (activity injected automatically)
//   - callback fn -> Java calls back into Rust (registered as a native method)
//
// Compare this to the raw JNI equivalent in cedinia/src/file_picker_android.rs (~350 lines).
// With jni-high the entire bridge fits in the android_bridge! block below.
//
// JAVA SIDE: see examples/java/CediniaFilePicker.java
//
// BUILD:
//   # Compile the Java helper to DEX (in build.rs or a script):
//   javac -cp $ANDROID_SDK/platforms/android-34/android.jar java/CediniaFilePicker.java
//   d8 --output out/ CediniaFilePicker.class
//
//   # Then build the Android library:
//   cargo build --target aarch64-linux-android --features jni-high/native-activity

use jni_high::{android_bridge, AndroidContext, BridgeResultExt};

// The bridge declaration: one block replaces ~350 lines of raw JNI in cedinia.
// - dex = the compiled DEX bytes embedded at compile time
// - class FilePicker { ... } generates: struct FilePicker, impl FilePicker { ... }
android_bridge! {
    dex = include_bytes!(concat!(env!("OUT_DIR"), "/file_picker.dex")),

    class FilePicker {
        java_name = "CediniaFilePicker",

        // Rust calls these; `activity` is injected automatically as the first Java argument.
        static fn launch_pick_directory(start_path: &str, is_include: bool);
        static fn open_url(url: &str);
        static fn open_file(path: &str);
        static fn open_folder(path: &str);

        // Java calls this back into Rust.
        // jni-high registers it as a native method - no #[no_mangle] or Java_* exports needed.
        callback fn on_directory_picked(path: String, is_include: bool);
    }
}

// --- App initialization -------------------------------------------------------

pub fn init(app: android_activity::AndroidApp) {
    // One-time setup. DEX loading and native registration happen lazily on first use.
    AndroidContext::init(app);

    // Register the callback handler. Can be changed at any time, even from another thread.
    FilePicker::set_on_directory_picked(|path, is_include| {
        if is_include {
            log::info!("User picked include directory: {path}");
            crate::app::add_include_dir(path);
        } else {
            log::info!("User picked exclude directory: {path}");
            crate::app::add_exclude_dir(path);
        }
    });
}

// --- Call sites ---------------------------------------------------------------
// Each call is one line. Error handling uses .log_err() from BridgeResultExt.

pub fn launch_pick_include_dir(start_path: &str) {
    FilePicker::launch_pick_directory(start_path, true).log_err("launch_pick_include_dir");
}

pub fn launch_pick_exclude_dir(start_path: &str) {
    FilePicker::launch_pick_directory(start_path, false).log_err("launch_pick_exclude_dir");
}

pub fn open_url(url: &str) {
    FilePicker::open_url(url).log_err("open_url");
}

pub fn open_file(path: &str) {
    FilePicker::open_file(path).log_err("open_file");
}

pub fn open_folder(path: &str) {
    FilePicker::open_folder(path).log_err("open_folder");
}

// ---------------------------------------------------------------------------
// WHAT THE COMPILER GENERATES (for reference, not user-written):
// ---------------------------------------------------------------------------
//
// pub struct FilePicker;
//
// impl FilePicker {
//     // Cached after first use - no findClass on every call
//     fn __class(ctx: &AndroidContext) -> BridgeResult<&'static Global<JClass<'static>>> { ... }
//
//     // Typed static method - activity is the hidden first arg
//     pub fn launch_pick_directory(start_path: &str, is_include: bool) -> BridgeResult<()> {
//         let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
//         ctx.attach(|env, activity| {
//             let __class = Self::__class(ctx)?;
//             let __j_start_path = env.new_string(start_path)?;
//             let __ret = env.call_static_method(
//                 __class, "launchPickDirectory",
//                 "(Landroid/app/Activity;Ljava/lang/String;Z)V",
//                 &[JValue::Object(activity), JValue::Object(&*__j_start_path), JValue::Bool(is_include as u8)],
//             )?;
//             Ok(())
//         })
//     }
//
//     // Callback setter - stores the handler in a static Mutex
//     pub fn set_on_directory_picked<F>(handler: F)
//     where F: Fn(String, bool) + Send + Sync + 'static { ... }
// }
//
// // Registered as native "onDirectoryPicked" on the CediniaFilePicker class
// unsafe extern "system" fn __filepicker_on_directory_picked_native<'local>(
//     mut __unowned: EnvUnowned<'local>,
//     _class: JClass<'local>,
//     path: JString<'local>,
//     is_include: jboolean,
// ) { ... }

// Stub for the example to reference
mod app {
    pub fn add_include_dir(_path: String) {}
    pub fn add_exclude_dir(_path: String) {}
}
