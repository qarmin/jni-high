use jni::objects::{JObject, JString, JValue};
use jni::signature::RuntimeMethodSignature;
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// Copy `text` to the system clipboard under the given `label`.
///
/// On Android 10+ (API 29) the clipboard is writable only while the app is in the foreground.
pub fn set_text(label: &str, text: &str) -> BridgeResult<()> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let svc: JObject = env.new_string("clipboard")?.into();
        let cm: JObject = env
            .call_method(
                activity,
                jni_str!("getSystemService"),
                jni_sig!((name: java.lang.String) -> java.lang.Object),
                &[JValue::Object(&svc)],
            )?
            .l()?;
        if cm.is_null() {
            return Ok(());
        }
        let j_label: JObject = env.new_string(label)?.into();
        let j_text: JObject = env.new_string(text)?.into();
        let clip: JObject = env
            .call_static_method(
                jni_str!("android/content/ClipData"),
                jni_str!("newPlainText"),
                jni_sig!((label: java.lang.CharSequence, text: java.lang.CharSequence) -> android.content.ClipData),
                &[JValue::Object(&j_label), JValue::Object(&j_text)],
            )?
            .l()?;
        env.call_method(
            &cm,
            jni_str!("setPrimaryClip"),
            jni_sig!((clip: android.content.ClipData) -> void),
            &[JValue::Object(&clip)],
        )?;
        Ok(())
    })
}

/// Read plain text from the system clipboard.
///
/// Returns `None` when the clipboard is empty or the item contains no text.
/// On Android 10+ (API 29) reading is only allowed while the app is focused.
pub fn get_text() -> BridgeResult<Option<String>> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let svc: JObject = env.new_string("clipboard")?.into();
        let cm: JObject = env
            .call_method(
                activity,
                jni_str!("getSystemService"),
                jni_sig!((name: java.lang.String) -> java.lang.Object),
                &[JValue::Object(&svc)],
            )?
            .l()?;
        if cm.is_null() {
            return Ok(None);
        }
        let has: bool = env
            .call_method(&cm, jni_str!("hasPrimaryClip"), jni_sig!(() -> boolean), &[])?
            .z()?;
        if !has {
            return Ok(None);
        }
        let clip: JObject = env
            .call_method(
                &cm,
                jni_str!("getPrimaryClip"),
                jni_sig!(() -> android.content.ClipData),
                &[],
            )?
            .l()?;
        if clip.is_null() {
            return Ok(None);
        }
        // ClipData.getItemAt(int) returns ClipData.Item (inner class - needs $)
        let sig_get_item =
            RuntimeMethodSignature::from_str("(I)Landroid/content/ClipData$Item;").expect("valid JNI signature");
        let item: JObject = env
            .call_method(
                &clip,
                jni_str!("getItemAt"),
                &sig_get_item.method_signature(),
                &[JValue::Int(0)],
            )?
            .l()?;
        if item.is_null() {
            return Ok(None);
        }
        let text_cs: JObject = env
            .call_method(&item, jni_str!("getText"), jni_sig!(() -> java.lang.CharSequence), &[])?
            .l()?;
        if text_cs.is_null() {
            return Ok(None);
        }
        let text_str: JObject = env
            .call_method(&text_cs, jni_str!("toString"), jni_sig!(() -> java.lang.String), &[])?
            .l()?;
        if text_str.is_null() {
            return Ok(None);
        }
        let jstr: JString = unsafe { JString::from_raw(env, text_str.as_raw()) };
        Ok(Some(jstr.try_to_string(env)?))
    })
}
