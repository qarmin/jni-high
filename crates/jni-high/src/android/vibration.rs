use jni::objects::{JObject, JValue};
use jni::signature::{FieldSignature, RuntimeFieldSignature, RuntimeMethodSignature};
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// Vibrate for `duration_ms` milliseconds.
///
/// Uses `VibrationEffect.createOneShot` on API >= 26, falls back to `Vibrator.vibrate(long)` on older.
/// Requires `android.permission.VIBRATE` in the manifest.
pub fn vibrate(duration_ms: u64) -> BridgeResult<()> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let vibrator = get_vibrator(env, activity)?;
        if !has_vibrator(env, &vibrator)? {
            return Err(BridgeError::NullPointer {
                context: "hasVibrator() returned false",
            });
        }
        let sdk = sdk_version(env)?;
        if sdk >= 26 {
            vibrate_with_effect(env, &vibrator, duration_ms)
        } else {
            vibrate_legacy(env, &vibrator, duration_ms)
        }
    })
}

/// Single haptic click using a predefined system effect on API >= 29, falls back to 50 ms one-shot.
///
/// Predefined effects (`EFFECT_HEAVY_CLICK`) are tuned per-device by the OEM and tend to work
/// reliably even on devices where `createOneShot` is suppressed.
/// Requires `android.permission.VIBRATE` in the manifest.
pub fn click() -> BridgeResult<()> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let vibrator = get_vibrator(env, activity)?;
        if !has_vibrator(env, &vibrator)? {
            return Err(BridgeError::NullPointer {
                context: "hasVibrator() returned false",
            });
        }
        let sdk = sdk_version(env)?;
        if sdk >= 29 {
            const EFFECT_HEAVY_CLICK: i32 = 5;
            vibrate_predefined(env, &vibrator, EFFECT_HEAVY_CLICK)
        } else if sdk >= 26 {
            vibrate_with_effect(env, &vibrator, 50)
        } else {
            vibrate_legacy(env, &vibrator, 50)
        }
    })
}

fn vibrate_predefined(env: &mut jni::Env<'_>, vibrator: &JObject<'_>, effect_id: i32) -> BridgeResult<()> {
    let sig_create = RuntimeMethodSignature::from_str("(I)Landroid/os/VibrationEffect;").expect("valid JNI signature");
    let effect: JObject = env
        .call_static_method(
            jni_str!("android/os/VibrationEffect"),
            jni_str!("createPredefined"),
            &sig_create.method_signature(),
            &[JValue::Int(effect_id)],
        )?
        .l()?;
    if effect.is_null() {
        return Err(BridgeError::NullPointer {
            context: "VibrationEffect.createPredefined returned null",
        });
    }
    let sig_vibrate = RuntimeMethodSignature::from_str("(Landroid/os/VibrationEffect;)V").expect("valid JNI signature");
    env.call_method(
        vibrator,
        jni_str!("vibrate"),
        &sig_vibrate.method_signature(),
        &[JValue::Object(&effect)],
    )?;
    Ok(())
}

/// Returns a diagnostic string useful when `vibrate()` succeeds but no vibration is felt.
///
/// Reports: SDK, hasVibrator, hasAmplitudeControl, VIBRATE permission, and ringer mode.
/// Ringer mode SILENT (0) or device-level "Vibration & haptics" off will suppress vibration
/// even when the API call succeeds.
pub fn debug_info() -> BridgeResult<String> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let sdk = sdk_version(env)?;
        let vibrator = match get_vibrator(env, activity) {
            Ok(v) => v,
            Err(_) => return Ok(format!("SDK={sdk} vibrator-svc=unavailable")),
        };
        let has_vib = has_vibrator(env, &vibrator).unwrap_or(false);

        let amp_ctrl = if sdk >= 26 {
            env.call_method(&vibrator, jni_str!("hasAmplitudeControl"), jni_sig!(() -> boolean), &[])
                .and_then(|v| v.z())
                .unwrap_or(false)
        } else {
            false
        };

        let perm_name: JObject = env.new_string("android.permission.VIBRATE")?.into();
        let check_sig = RuntimeMethodSignature::from_str("(Ljava/lang/String;)I").expect("valid JNI signature");
        let perm_ok = env
            .call_method(
                activity,
                jni_str!("checkSelfPermission"),
                &check_sig.method_signature(),
                &[JValue::Object(&perm_name)],
            )?
            .i()?
            == 0;

        let audio_name: JObject = env.new_string("audio")?.into();
        let audio: JObject = env
            .call_method(
                activity,
                jni_str!("getSystemService"),
                jni_sig!((name: java.lang.String) -> java.lang.Object),
                &[JValue::Object(&audio_name)],
            )?
            .l()?;
        let ringer = if audio.is_null() {
            "?".to_owned()
        } else {
            match env
                .call_method(&audio, jni_str!("getRingerMode"), jni_sig!(() -> int), &[])?
                .i()?
            {
                0 => "SILENT".to_owned(),
                1 => "VIBRATE".to_owned(),
                2 => "NORMAL".to_owned(),
                n => format!("{n}"),
            }
        };

        Ok(format!(
            "SDK={sdk} hasVibrator={has_vib} ampCtrl={amp_ctrl} VIBRATE={} ringer={ringer}",
            if perm_ok { "ok" } else { "DENIED!" }
        ))
    })
}

fn get_vibrator<'env>(env: &mut jni::Env<'env>, activity: &JObject<'_>) -> BridgeResult<JObject<'env>> {
    let svc: JObject = env.new_string("vibrator")?.into();
    let vibrator: JObject = env
        .call_method(
            activity,
            jni_str!("getSystemService"),
            jni_sig!((name: java.lang.String) -> java.lang.Object),
            &[JValue::Object(&svc)],
        )?
        .l()?;
    if vibrator.is_null() {
        return Err(BridgeError::NullPointer {
            context: "getSystemService(vibrator)",
        });
    }
    Ok(vibrator)
}

fn has_vibrator(env: &mut jni::Env<'_>, vibrator: &JObject<'_>) -> BridgeResult<bool> {
    Ok(env
        .call_method(vibrator, jni_str!("hasVibrator"), jni_sig!(() -> boolean), &[])?
        .z()?)
}

fn vibrate_with_effect(env: &mut jni::Env<'_>, vibrator: &JObject<'_>, duration_ms: u64) -> BridgeResult<()> {
    // Use explicit max amplitude (255) rather than DEFAULT_AMPLITUDE (-1).
    // On some devices DEFAULT_AMPLITUDE resolves to near-zero, causing silent no-ops.
    const AMPLITUDE_MAX: i32 = 255;
    let sig_create = RuntimeMethodSignature::from_str("(JI)Landroid/os/VibrationEffect;").expect("valid JNI signature");
    let effect: JObject = env
        .call_static_method(
            jni_str!("android/os/VibrationEffect"),
            jni_str!("createOneShot"),
            &sig_create.method_signature(),
            &[JValue::Long(duration_ms as i64), JValue::Int(AMPLITUDE_MAX)],
        )?
        .l()?;
    if effect.is_null() {
        return Err(BridgeError::NullPointer {
            context: "VibrationEffect.createOneShot returned null",
        });
    }
    let sig_vibrate = RuntimeMethodSignature::from_str("(Landroid/os/VibrationEffect;)V").expect("valid JNI signature");
    env.call_method(
        vibrator,
        jni_str!("vibrate"),
        &sig_vibrate.method_signature(),
        &[JValue::Object(&effect)],
    )?;
    Ok(())
}

fn vibrate_legacy(env: &mut jni::Env<'_>, vibrator: &JObject<'_>, duration_ms: u64) -> BridgeResult<()> {
    let sig = RuntimeMethodSignature::from_str("(J)V").expect("valid JNI signature");
    env.call_method(
        vibrator,
        jni_str!("vibrate"),
        &sig.method_signature(),
        &[JValue::Long(duration_ms as i64)],
    )?;
    Ok(())
}

pub(crate) fn sdk_version(env: &mut jni::Env<'_>) -> BridgeResult<i32> {
    let rfs = RuntimeFieldSignature::from_str("I").expect("valid JNI primitive descriptor");
    let int_sig = FieldSignature::from(&rfs);
    Ok(env
        .get_static_field(jni_str!("android/os/Build$VERSION"), jni_str!("SDK_INT"), &int_sig)?
        .i()?)
}
