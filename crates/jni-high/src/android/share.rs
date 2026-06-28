use jni::objects::{JObject, JValue};
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// Open the system share sheet to share plain text.
///
/// `subject` is optional (used by email clients and similar apps).
/// Must be called from the main/UI thread - `startActivity` is a UI operation.
pub fn text(subject: Option<&str>, body: &str) -> BridgeResult<()> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let action: JObject = env.new_string("android.intent.action.SEND")?.into();
        let intent: JObject = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!((action: java.lang.String) -> void),
            &[JValue::Object(&action)],
        )?;
        let mime: JObject = env.new_string("text/plain")?.into();
        env.call_method(
            &intent,
            jni_str!("setType"),
            jni_sig!((type_: java.lang.String) -> android.content.Intent),
            &[JValue::Object(&mime)],
        )?;
        if let Some(subj) = subject {
            let key: JObject = env.new_string("android.intent.extra.SUBJECT")?.into();
            let val: JObject = env.new_string(subj)?.into();
            env.call_method(
                &intent,
                jni_str!("putExtra"),
                jni_sig!((name: java.lang.String, value: java.lang.String) -> android.content.Intent),
                &[JValue::Object(&key), JValue::Object(&val)],
            )?;
        }
        let key: JObject = env.new_string("android.intent.extra.TEXT")?.into();
        let val: JObject = env.new_string(body)?.into();
        env.call_method(
            &intent,
            jni_str!("putExtra"),
            jni_sig!((name: java.lang.String, value: java.lang.String) -> android.content.Intent),
            &[JValue::Object(&key), JValue::Object(&val)],
        )?;
        let chooser: JObject = env
            .call_static_method(
                jni_str!("android/content/Intent"),
                jni_str!("createChooser"),
                jni_sig!((target: android.content.Intent, title: java.lang.CharSequence) -> android.content.Intent),
                &[JValue::Object(&intent), JValue::Object(&JObject::null())],
            )?
            .l()?;
        env.call_method(
            activity,
            jni_str!("startActivity"),
            jni_sig!((intent: android.content.Intent) -> void),
            &[JValue::Object(&chooser)],
        )?;
        Ok(())
    })
}
