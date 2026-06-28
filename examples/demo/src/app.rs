use slint::ComponentHandle;

use crate::{Demo, MainWindow};

pub fn run_app() {
    let window = MainWindow::new().expect("MainWindow::new failed");
    wire_callbacks(&window);
    window.run().expect("event loop failed");
}

fn wire_callbacks(window: &MainWindow) {
    wire_clipboard(window);
    wire_share(window);
    wire_system_info(window);
    wire_haptics(window);
    wire_permissions(window);
    wire_notifications(window);
    wire_browser(window);
}

fn wire_clipboard(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Demo>().on_clipboard_set(move || {
        #[cfg(target_os = "android")]
        let result = jni_high::android::clipboard::set_text("jni-high", "Hello from jni-high!");
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<()> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Clipboard write", result.map(|()| "written 'Hello from jni-high!'".into()));
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_clipboard_get(move || {
        #[cfg(target_os = "android")]
        let result = jni_high::android::clipboard::get_text()
            .map(|opt| opt.unwrap_or_else(|| "(empty)".into()));
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Clipboard read", result);
    });
}

fn wire_share(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Demo>().on_share_text(move || {
        #[cfg(target_os = "android")]
        let result =
            jni_high::android::share::text(Some("jni-high demo"), "Shared from the jni-high demo app!")
                .map(|()| "share sheet opened".into());
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Share", result);
    });
}

fn wire_system_info(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Demo>().on_get_locale(move || {
        #[cfg(target_os = "android")]
        let result = jni_high::android::locale::system_locale_tag();
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Locale", result);
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_check_connectivity(move || {
        #[cfg(target_os = "android")]
        let result = jni_high::android::connectivity::active_type().map(|t| format!("{t:?}"));
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Connectivity", result);
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_get_device_info(move || {
        #[cfg(target_os = "android")]
        let result = jni_high::android::device::info().map(|d| {
            format!("{} {} Android {} (SDK {})", d.manufacturer, d.model, d.android_version, d.sdk_int)
        });
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Device", result);
    });
}

fn wire_haptics(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Demo>().on_vibrate(move || {
        #[cfg(target_os = "android")]
        {
            let info = jni_high::android::vibration::debug_info().unwrap_or_else(|e| format!("diag-err:{e}"));
            let result = jni_high::android::vibration::vibrate(300).map(|()| format!("vibrating 300ms [{info}]"));
            log_result(&weak, "Vibration", result);
        }
        #[cfg(not(target_os = "android"))]
        log_result(&weak, "Vibration", Err(jni_high::BridgeError::ContextNotInitialized));
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_haptic_click(move || {
        #[cfg(target_os = "android")]
        {
            let result = jni_high::android::vibration::click().map(|()| "predefined HEAVY_CLICK sent".into());
            log_result(&weak, "Haptic click", result);
        }
        #[cfg(not(target_os = "android"))]
        log_result(&weak, "Haptic click", Err(jni_high::BridgeError::ContextNotInitialized));
    });
}

fn wire_permissions(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Demo>().on_check_notification_permission(move || {
        #[cfg(target_os = "android")]
        let result =
            jni_high::android::permissions::is_granted(jni_high::android::permissions::POST_NOTIFICATIONS)
                .map(|granted| if granted { "GRANTED" } else { "DENIED" }.into());
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "POST_NOTIFICATIONS", result);
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_check_storage_permission(move || {
        #[cfg(target_os = "android")]
        {
            let legacy = jni_high::android::permissions::is_granted(jni_high::android::permissions::READ_EXTERNAL_STORAGE);
            let media = jni_high::android::permissions::is_granted(jni_high::android::permissions::READ_MEDIA_IMAGES);
            let msg: jni_high::BridgeResult<String> = match (legacy, media) {
                (Ok(true), _) => Ok("READ_EXTERNAL_STORAGE=GRANTED".into()),
                (_, Ok(true)) => Ok("READ_MEDIA_IMAGES=GRANTED (API33+)".into()),
                (Ok(false), Ok(false)) => Ok("both DENIED - use Request button".into()),
                (Err(e), _) | (_, Err(e)) => Err(e),
            };
            log_result(&weak, "Storage", msg);
        }
        #[cfg(not(target_os = "android"))]
        log_result(&weak, "Storage", Err(jni_high::BridgeError::ContextNotInitialized));
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_request_storage_permission(move || {
        #[cfg(target_os = "android")]
        {
            let perms = &[
                jni_high::android::permissions::READ_EXTERNAL_STORAGE,
                jni_high::android::permissions::READ_MEDIA_IMAGES,
            ];
            let result = jni_high::android::permissions::request(perms, 101)
                .map(|()| "dialog opened - re-check after granting".into());
            log_result(&weak, "Storage request", result);
        }
        #[cfg(not(target_os = "android"))]
        log_result(&weak, "Storage", Err(jni_high::BridgeError::ContextNotInitialized));
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_check_camera_permission(move || {
        #[cfg(target_os = "android")]
        {
            let cam = jni_high::android::permissions::is_granted(jni_high::android::permissions::CAMERA);
            let mic = jni_high::android::permissions::is_granted(jni_high::android::permissions::RECORD_AUDIO);
            let msg: jni_high::BridgeResult<String> = match (cam, mic) {
                (Ok(c), Ok(m)) => Ok(format!(
                    "CAMERA={} RECORD_AUDIO={}",
                    if c { "GRANTED" } else { "DENIED" },
                    if m { "GRANTED" } else { "DENIED" }
                )),
                (Err(e), _) | (_, Err(e)) => Err(e),
            };
            log_result(&weak, "Camera/mic", msg);
        }
        #[cfg(not(target_os = "android"))]
        log_result(&weak, "Camera/mic", Err(jni_high::BridgeError::ContextNotInitialized));
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_request_camera_permission(move || {
        #[cfg(target_os = "android")]
        {
            let perms = &[
                jni_high::android::permissions::CAMERA,
                jni_high::android::permissions::RECORD_AUDIO,
            ];
            let result = jni_high::android::permissions::request(perms, 102)
                .map(|()| "dialog opened - re-check after granting".into());
            log_result(&weak, "Camera/mic request", result);
        }
        #[cfg(not(target_os = "android"))]
        log_result(&weak, "Camera/mic", Err(jni_high::BridgeError::ContextNotInitialized));
    });
}

fn wire_notifications(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Demo>().on_open_notification_settings(move || {
        #[cfg(target_os = "android")]
        let result = jni_high::android::notifications::open_settings().map(|()| "settings opened".into());
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Notification settings", result);
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_send_notification(move || {
        #[cfg(target_os = "android")]
        {
            match jni_high::android::notifications::are_enabled() {
                Ok(false) => {
                    log_append(&weak, "Notification: permission not granted - tap 'Open settings'");
                    return;
                }
                Err(e) => {
                    log_append(&weak, &format!("Notification: permission check failed - {e}"));
                    return;
                }
                Ok(true) => {}
            }
            let result = jni_high::android::notifications::try_send(
                "jni-high Demo",
                "Test notification from jni-high!",
                "jni_high_demo",
                "Demo notifications",
                1,
            )
            .map(|()| "notification sent".into());
            log_result(&weak, "Notification", result);
        }
        #[cfg(not(target_os = "android"))]
        log_result(&weak, "Notification", Err(jni_high::BridgeError::ContextNotInitialized));
    });
}

fn wire_browser(window: &MainWindow) {
    let weak = window.as_weak();
    window.global::<Demo>().on_open_url(move || {
        #[cfg(target_os = "android")]
        let result = jni_high::android::browser::open_url("https://github.com/rafalmag/jni-high")
            .map(|()| "browser opened".into());
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Open URL", result);
    });

    let weak = window.as_weak();
    window.global::<Demo>().on_open_camera(move || {
        #[cfg(target_os = "android")]
        let result = jni_high::android::browser::open_camera().map(|()| "camera app opened".into());
        #[cfg(not(target_os = "android"))]
        let result: jni_high::BridgeResult<String> = Err(jni_high::BridgeError::ContextNotInitialized);
        log_result(&weak, "Camera", result);
    });
}

fn log_result(weak: &slint::Weak<MainWindow>, label: &str, result: jni_high::BridgeResult<String>) {
    let msg = match result {
        Ok(val) => format!("{label}: {val}"),
        Err(jni_high::BridgeError::ContextNotInitialized) => format!("{label}: not available on desktop"),
        Err(e) => format!("{label}: ERROR - {e}"),
    };
    log_append(weak, &msg);
}

fn log_append(weak: &slint::Weak<MainWindow>, msg: &str) {
    let weak = weak.clone();
    let msg = msg.to_owned();
    slint::invoke_from_event_loop(move || {
        let Some(w) = weak.upgrade() else {
            log::warn!("log_append: window closed before log update");
            return;
        };
        let demo = w.global::<Demo>();
        let old = demo.get_log().to_string();
        let mut lines: Vec<String> = old.lines().map(str::to_owned).collect();
        lines.insert(0, msg);
        lines.truncate(6);
        demo.set_log(lines.join("\n").into());
    })
    .ok();
}
