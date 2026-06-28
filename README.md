# jni-high

A high-level Rust <-> Android/Java bridge built on top of [`jni-rs`](https://github.com/jni-rs/jni-rs).

Instead of hand-writing JNI calls (`FindClass`, `GetStaticMethodID`, signature strings, `#[no_mangle]` exports, ...), you describe the Java surface once in a declarative macro and get typed Rust functions back.

Raw JNI code tends to accumulate `unsafe` blocks and easy-to-get-wrong logic (manual signature strings, manual global-ref lifetime management, manual native-method registration). The goal of `jni-high` is to push all of that into one audited place, so calling into Java from application code stays short, safe, and hard to misuse.

## Quick example

```rust
use jni_high::{android_bridge, AndroidContext, BridgeResultExt};

android_bridge! {
    dex = include_bytes!(concat!(env!("OUT_DIR"), "/file_picker.dex")),

    class FilePicker {
        java_name = "CediniaFilePicker",

        // Rust calls this; the current Activity is injected automatically.
        static fn launch_pick_directory(start_path: &str, is_include: bool);

        // A method that doesn't need an Activity at all.
        #[no_activity]
        static fn are_enabled() -> bool;

        // Java calls back into Rust - registered as a native method, no
        // #[no_mangle] or Java_* export required.
        callback fn on_directory_picked(path: String, is_include: bool);
    }
}

fn init(app: android_activity::AndroidApp) {
    AndroidContext::init(app);
    FilePicker::set_on_directory_picked(|path, is_include| {
        log::info!("picked {path} (include={is_include})");
    });
}

fn pick_dir(start_path: &str) {
    FilePicker::launch_pick_directory(start_path, true).log_err("pick_dir");
}
```

The `dex` field embeds a pre-compiled `.dex` file (compiled from a small Java/Kotlin helper class with `javac` + `d8`) directly into the binary; it is loaded and its class cached on first use. See `examples/` for the full set of runnable snippets and `examples/java/` for the matching Java sources.

## What's in the box

- **`android_bridge!` macro** - generates typed static-method calls and native-method callbacks from a short declarative block (see above).
- **Built-in helpers** for common Android APIs, usable without writing any bridge code(this li:

  | Module          | Purpose                                             |
  |------------------|------------------------------------------------------|
  | `android::activity`      | App-private files/cache directories             |
  | `android::browser`       | Open a URL in the system browser                |
  | `android::clipboard`     | Read/write the system clipboard                 |
  | `android::connectivity`  | Current network type (Wi-Fi/mobile/none)        |
  | `android::device`        | Manufacturer, model, Android/SDK version        |
  | `android::locale`        | System language / locale                        |
  | `android::notifications` | Post notifications, check permission            |
  | `android::permissions`   | Common runtime permission constants + checks    |
  | `android::share`         | System share sheet                              |
  | `android::vibration`     | Vibrate the device(this currently not works)                              |

- **`AndroidContext`** - a small singleton wrapping the `JavaVM` and current `Activity`, set up once via `AndroidContext::init(app)` and reused by every call.
- **`BridgeError`/`BridgeResult`** - a typed error enum (`ContextNotInitialized`, `VmNotAvailable`, `JavaException { .. }`, ...) instead of raw `jni::errors::Error`.

## Workspace layout

| Crate               | Role                                                        |
|---------------------|--------------------------------------------------------------|
| `jni-high`          | Runtime: `AndroidContext`, built-in Android helpers, error types |
| `jni-high-macros`   | The `android_bridge!` proc macro                             |
| `jni-high-build`    | Build-script helper for compiling Java/Kotlin sources to DEX (WIP) |

All three are published on crates.io as independent, versioned crates, so you depend on them the same way as any other crate - no `path` dependency or git checkout required:

```toml
[dependencies]
jni-high = "0.1.0"
```

## Used by

[Cedinia](https://github.com/qarmin/czkawka/tree/master/cedinia), the Android frontend of [Czkawka](https://github.com/qarmin/czkawka), uses `jni-high` for its Android integration (file picker, notifications, locale, clipboard, sharing, ...). Comparing [`cedinia/src/file_picker_android.rs`](https://github.com/qarmin/czkawka/blob/master/cedinia/src/file_picker_android.rs) (raw JNI, ~350 lines) to `examples/01_file_picker.rs` in this repo (one `android_bridge!` block) is a good illustration of what the macro replaces.

## Compilation

Building for Android requires the Android NDK/SDK and `cargo-apk`; see the `justfile` for the exact commands used to build, install and run the demo app (`just demo`, `just run`, `just log`, ...).

```shell
cargo check --workspace                 # desktop-only sanity check (Android modules are cfg-gated)
cargo build --target aarch64-linux-android --features jni-high/native-activity
```

## AI usage

This project was created, with AI assistance.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
