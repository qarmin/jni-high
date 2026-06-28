mod app;
slint::include_modules!();
pub use app::run_app;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: slint::android::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("jni-high-demo"),
    );
    log::info!("android_main: starting");
    jni_high::AndroidContext::init(android_app.clone());
    slint::android::init(android_app).expect("Slint android init failed");
    asan_smoketest_if_requested();
    run_app();
}

// Fallback ASAN options if the runtime cannot read ASAN_OPTIONS from the environment.
// allow_user_segv_handler keeps Slint's signal handling working under ASAN.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn __asan_default_options() -> *const std::ffi::c_char {
    b"allow_user_segv_handler=1:log_to_syslog=false:detect_leaks=0:abort_on_error=1\0".as_ptr() as *const _
}

// Triggers a deliberate heap-buffer-overflow when triggered by `just demo_asan smoke`.
// Checks an env var (set via wrap.sh if available) OR a flag file pushed by adb.
#[cfg(target_os = "android")]
fn asan_smoketest_if_requested() {
    const FLAG_FILE: &str = "/data/local/tmp/jni_high_asan_smoketest";
    let triggered = std::env::var_os("JNIHIGH_DEMO_ASAN_SMOKETEST").is_some()
        || std::path::Path::new(FLAG_FILE).exists();
    if !triggered {
        return;
    }
    let _ = std::fs::remove_file(FLAG_FILE);
    log::error!("ASAN SMOKETEST: triggering a deliberate heap-buffer-overflow now");
    let v: Vec<u8> = vec![0xAB; 4];
    let ptr = v.as_ptr();
    let offset = std::hint::black_box(64usize);
    // Safety: intentionally out-of-bounds to trigger ASan.
    let byte = unsafe { std::ptr::read_volatile(ptr.add(offset)) };
    std::hint::black_box(byte);
    log::error!("ASAN SMOKETEST: still alive at byte={byte:#x} - ASan is NOT active in this build");
}
