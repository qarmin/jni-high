use jni::objects::{JObject, JValue};
use jni::signature::{FieldSignature, RuntimeFieldSignature, RuntimeMethodSignature};
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// Returns `true` when the system has not blocked notifications for this app.
///
/// Uses `NotificationManager.areNotificationsEnabled()`. Returns `true` on any
/// JNI error so callers fail open rather than silently suppressing notifications.
pub fn are_enabled() -> BridgeResult<bool> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let svc_name: JObject = env.new_string("notification")?.into();
        let nm: JObject = env
            .call_method(
                activity,
                jni_str!("getSystemService"),
                jni_sig!((name: java.lang.String) -> java.lang.Object),
                &[JValue::Object(&svc_name)],
            )?
            .l()?;
        if nm.is_null() {
            return Ok(true);
        }
        let enabled: bool = env
            .call_method(&nm, jni_str!("areNotificationsEnabled"), jni_sig!(() -> boolean), &[])?
            .z()?;
        Ok(enabled)
    })
}

/// Opens the system notification settings screen for this app.
pub fn open_settings() -> BridgeResult<()> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let pkg: JObject = env
            .call_method(
                activity,
                jni_str!("getPackageName"),
                jni_sig!(() -> java.lang.String),
                &[],
            )?
            .l()?;
        let action: JObject = env.new_string("android.settings.APP_NOTIFICATION_SETTINGS")?.into();
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!((action: java.lang.String) -> void),
            &[JValue::Object(&action)],
        )?;
        let key: JObject = env.new_string("android.provider.extra.APP_PACKAGE")?.into();
        env.call_method(
            &intent,
            jni_str!("putExtra"),
            jni_sig!((name: java.lang.String, value: java.lang.String) -> android.content.Intent),
            &[JValue::Object(&key), JValue::Object(&pkg)],
        )?;
        env.call_method(
            activity,
            jni_str!("startActivity"),
            jni_sig!((intent: android.content.Intent) -> void),
            &[JValue::Object(&intent)],
        )?;
        Ok(())
    })
}

/// Post a notification synchronously on the calling thread, returning any error.
///
/// Prefer this over `send()` when you need to surface JNI errors to the caller.
/// Blocks the calling thread until the notification is posted (or fails).
///
/// - `channel_id` / `channel_name` - Android O+ notification channel (idempotent creation).
/// - `notification_id` - same ID updates an existing notification instead of posting a new one.
pub fn try_send(
    title: &str,
    body: &str,
    channel_id: &str,
    channel_name: &str,
    notification_id: i32,
) -> BridgeResult<()> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| build_and_post(env, activity, title, body, channel_id, channel_name, notification_id))
}

/// Post a notification. Fire-and-forget: runs on a background thread, JNI errors are logged.
///
/// Use `try_send()` instead when you need the result on the calling thread.
pub fn send(title: &str, body: &str, channel_id: &str, channel_name: &str, notification_id: i32) {
    if AndroidContext::get().is_none() {
        log::warn!("notifications::send: context not initialized");
        return;
    }
    let (title, body, channel_id, channel_name) = (
        title.to_owned(),
        body.to_owned(),
        channel_id.to_owned(),
        channel_name.to_owned(),
    );
    std::thread::spawn(move || {
        if let Err(e) = try_send(&title, &body, &channel_id, &channel_name, notification_id) {
            log::warn!("notifications::send: {e:?}");
        }
    });
}

fn build_and_post(
    env: &mut jni::Env<'_>,
    activity: &JObject<'_>,
    title: &str,
    body: &str,
    channel_id: &str,
    channel_name: &str,
    notification_id: i32,
) -> BridgeResult<()> {
    let icon_id = launcher_icon_id(env, activity)?;
    let nm = create_channel_get_manager(env, activity, channel_id, channel_name)?;
    let builder = create_builder(env, activity, channel_id, icon_id, title, body)?;
    if let Some(pending) = launch_pending_intent(env, activity)? {
        let sig = RuntimeMethodSignature::from_str("(Landroid/app/PendingIntent;)Landroid/app/Notification$Builder;")
            .expect("valid JNI signature");
        env.call_method(
            &builder,
            jni_str!("setContentIntent"),
            &sig.method_signature(),
            &[JValue::Object(&pending)],
        )?;
    }
    let build_sig = RuntimeMethodSignature::from_str("()Landroid/app/Notification;").expect("valid JNI signature");
    let notification: JObject = env
        .call_method(&builder, jni_str!("build"), &build_sig.method_signature(), &[])?
        .l()?;
    env.call_method(
        &nm,
        jni_str!("notify"),
        jni_sig!((id: int, notification: android.app.Notification) -> void),
        &[JValue::Int(notification_id), JValue::Object(&notification)],
    )?;
    Ok(())
}

fn create_channel_get_manager<'env>(
    env: &mut jni::Env<'env>,
    activity: &JObject<'_>,
    channel_id: &str,
    channel_name: &str,
) -> BridgeResult<JObject<'env>> {
    let j_channel_id: JObject = env.new_string(channel_id)?.into();
    let j_channel_name: JObject = env.new_string(channel_name)?.into();
    let channel = env.new_object(
        jni_str!("android/app/NotificationChannel"),
        jni_sig!((id: java.lang.String, name: java.lang.CharSequence, importance: int) -> void),
        &[
            JValue::Object(&j_channel_id),
            JValue::Object(&j_channel_name),
            JValue::Int(3),
        ],
    )?;
    let svc_name: JObject = env.new_string("notification")?.into();
    let nm: JObject = env
        .call_method(
            activity,
            jni_str!("getSystemService"),
            jni_sig!((name: java.lang.String) -> java.lang.Object),
            &[JValue::Object(&svc_name)],
        )?
        .l()?;
    if nm.is_null() {
        return Err(BridgeError::NullPointer {
            context: "getSystemService(notification)",
        });
    }
    env.call_method(
        &nm,
        jni_str!("createNotificationChannel"),
        jni_sig!((channel: android.app.NotificationChannel) -> void),
        &[JValue::Object(&channel)],
    )?;
    Ok(nm)
}

fn create_builder<'env>(
    env: &mut jni::Env<'env>,
    activity: &JObject<'_>,
    channel_id: &str,
    icon_id: i32,
    title: &str,
    body: &str,
) -> BridgeResult<JObject<'env>> {
    let j_channel_id: JObject = env.new_string(channel_id)?.into();
    let ctor_sig = RuntimeMethodSignature::from_str("(Landroid/content/Context;Ljava/lang/String;)V")
        .expect("valid JNI signature");
    let builder = env.new_object(
        jni_str!("android/app/Notification$Builder"),
        &ctor_sig.method_signature(),
        &[JValue::Object(activity), JValue::Object(&j_channel_id)],
    )?;
    let set_cs = RuntimeMethodSignature::from_str("(Ljava/lang/CharSequence;)Landroid/app/Notification$Builder;")
        .expect("valid JNI signature");
    let j_title: JObject = env.new_string(title)?.into();
    env.call_method(
        &builder,
        jni_str!("setContentTitle"),
        &set_cs.method_signature(),
        &[JValue::Object(&j_title)],
    )?;
    let j_body: JObject = env.new_string(body)?.into();
    env.call_method(
        &builder,
        jni_str!("setContentText"),
        &set_cs.method_signature(),
        &[JValue::Object(&j_body)],
    )?;
    let set_icon =
        RuntimeMethodSignature::from_str("(I)Landroid/app/Notification$Builder;").expect("valid JNI signature");
    env.call_method(
        &builder,
        jni_str!("setSmallIcon"),
        &set_icon.method_signature(),
        &[JValue::Int(icon_id)],
    )?;
    let set_bool =
        RuntimeMethodSignature::from_str("(Z)Landroid/app/Notification$Builder;").expect("valid JNI signature");
    env.call_method(
        &builder,
        jni_str!("setAutoCancel"),
        &set_bool.method_signature(),
        &[JValue::Bool(true)],
    )?;
    Ok(builder)
}

fn launcher_icon_id(env: &mut jni::Env<'_>, activity: &JObject<'_>) -> BridgeResult<i32> {
    let pkg: JObject = env
        .call_method(
            activity,
            jni_str!("getPackageName"),
            jni_sig!(() -> java.lang.String),
            &[],
        )?
        .l()?;

    // Primary: mipmap/ic_launcher via Resources.getIdentifier.
    let resources: JObject = env
        .call_method(
            activity,
            jni_str!("getResources"),
            jni_sig!(() -> android.content.res.Resources),
            &[],
        )?
        .l()?;
    let icon_name = env.new_string("ic_launcher")?;
    let mipmap_type = env.new_string("mipmap")?;
    let id: i32 = env
        .call_method(
            &resources,
            jni_str!("getIdentifier"),
            jni_sig!((name: java.lang.String, defType: java.lang.String, defPackage: java.lang.String) -> int),
            &[
                JValue::Object(&icon_name),
                JValue::Object(&mipmap_type),
                JValue::Object(&pkg),
            ],
        )?
        .i()?;
    if id != 0 {
        return Ok(id);
    }

    // Fallback: ApplicationInfo.icon from PackageManager.
    let pm: JObject = env
        .call_method(
            activity,
            jni_str!("getPackageManager"),
            jni_sig!(() -> android.content.pm.PackageManager),
            &[],
        )?
        .l()?;
    let get_info_sig = RuntimeMethodSignature::from_str("(Ljava/lang/String;I)Landroid/content/pm/ApplicationInfo;")
        .expect("valid JNI signature");
    let app_info: JObject = env
        .call_method(
            &pm,
            jni_str!("getApplicationInfo"),
            &get_info_sig.method_signature(),
            &[JValue::Object(&pkg), JValue::Int(0)],
        )?
        .l()?;
    if !app_info.is_null() {
        let rfs = RuntimeFieldSignature::from_str("I").expect("valid JNI primitive descriptor");
        let int_sig = FieldSignature::from(&rfs);
        let icon = env.get_field(&app_info, jni_str!("icon"), &int_sig)?.i()?;
        if icon != 0 {
            return Ok(icon);
        }
    }

    // Last resort: android.R.drawable.sym_def_app_icon (stable since API 1).
    log::warn!("notifications: no app icon found, using system fallback");
    Ok(0x0108_0017_i32)
}

fn launch_pending_intent<'env>(
    env: &mut jni::Env<'env>,
    activity: &JObject<'_>,
) -> BridgeResult<Option<JObject<'env>>> {
    let pm: JObject = env
        .call_method(
            activity,
            jni_str!("getPackageManager"),
            jni_sig!(() -> android.content.pm.PackageManager),
            &[],
        )?
        .l()?;
    let pkg: JObject = env
        .call_method(
            activity,
            jni_str!("getPackageName"),
            jni_sig!(() -> java.lang.String),
            &[],
        )?
        .l()?;
    let launch_intent: JObject = env
        .call_method(
            &pm,
            jni_str!("getLaunchIntentForPackage"),
            jni_sig!((packageName: java.lang.String) -> android.content.Intent),
            &[JValue::Object(&pkg)],
        )?
        .l()?;
    if launch_intent.is_null() {
        return Ok(None);
    }
    const FLAG_SINGLE_TOP: i32 = 0x2000_0000;
    const FLAG_CLEAR_TOP: i32 = 0x0400_0000;
    // PendingIntent.FLAG_IMMUTABLE = 1<<26, required on Android 12+ (API 31).
    const FLAG_IMMUTABLE: i32 = 0x0400_0000;
    env.call_method(
        &launch_intent,
        jni_str!("addFlags"),
        jni_sig!((flags: int) -> android.content.Intent),
        &[JValue::Int(FLAG_SINGLE_TOP | FLAG_CLEAR_TOP)],
    )?;
    let sig = RuntimeMethodSignature::from_str(
        "(Landroid/content/Context;ILandroid/content/Intent;I)Landroid/app/PendingIntent;",
    )
    .expect("valid JNI signature");
    let pending: JObject = env
        .call_static_method(
            jni_str!("android/app/PendingIntent"),
            jni_str!("getActivity"),
            &sig.method_signature(),
            &[
                JValue::Object(activity),
                JValue::Int(0),
                JValue::Object(&launch_intent),
                JValue::Int(FLAG_IMMUTABLE),
            ],
        )?
        .l()?;
    Ok(if pending.is_null() { None } else { Some(pending) })
}
