use jni::objects::{JObject, JValue};
use jni::signature::RuntimeMethodSignature;
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// Open a URL in the system browser (or any app that handles `ACTION_VIEW`).
pub fn open_url(url: &str) -> BridgeResult<()> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let url_str: JObject = env.new_string(url)?.into();
        let parse_sig =
            RuntimeMethodSignature::from_str("(Ljava/lang/String;)Landroid/net/Uri;").expect("valid JNI signature");
        let uri: JObject = env
            .call_static_method(
                jni_str!("android/net/Uri"),
                jni_str!("parse"),
                &parse_sig.method_signature(),
                &[JValue::Object(&url_str)],
            )?
            .l()?;
        if uri.is_null() {
            return Err(BridgeError::NullPointer {
                context: "Uri.parse returned null",
            });
        }
        let action: JObject = env.new_string("android.intent.action.VIEW")?.into();
        let ctor_sig =
            RuntimeMethodSignature::from_str("(Ljava/lang/String;Landroid/net/Uri;)V").expect("valid JNI signature");
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            &ctor_sig.method_signature(),
            &[JValue::Object(&action), JValue::Object(&uri)],
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

/// Open the system camera app for image capture.
///
/// This uses `ACTION_IMAGE_CAPTURE` which starts the camera app and returns immediately.
/// NativeActivity does not support `startActivityForResult`, so the captured image cannot
/// be retrieved this way - use a DEX helper or a content provider for that.
///
/// Requires `android.permission.CAMERA` in the manifest and runtime grant on API >= 23.
pub fn open_camera() -> BridgeResult<()> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let action: JObject = env.new_string("android.media.action.IMAGE_CAPTURE")?.into();
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!((action: java.lang.String) -> void),
            &[JValue::Object(&action)],
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
