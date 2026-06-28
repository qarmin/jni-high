use jni::objects::{JObject, JString};
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// ISO 639-1 language code of the system default locale (e.g. `"en"`, `"pl"`, `"de"`).
pub fn system_language() -> BridgeResult<String> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, _activity| {
        let locale = default_locale(env)?;
        jni_string_method(env, &locale, jni_str!("getLanguage"))
    })
}

/// BCP 47 language tag of the system default locale (e.g. `"en-US"`, `"pl-PL"`, `"zh-Hans-CN"`).
pub fn system_locale_tag() -> BridgeResult<String> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, _activity| {
        let locale = default_locale(env)?;
        jni_string_method(env, &locale, jni_str!("toLanguageTag"))
    })
}

fn default_locale<'env>(env: &mut jni::Env<'env>) -> jni::errors::Result<JObject<'env>> {
    env.call_static_method(
        jni_str!("java/util/Locale"),
        jni_str!("getDefault"),
        jni_sig!(() -> java.util.Locale),
        &[],
    )?
    .l()
}

fn jni_string_method(env: &mut jni::Env<'_>, obj: &JObject<'_>, method: &jni::strings::JNIStr) -> BridgeResult<String> {
    let result: JObject = env
        .call_method(obj, method, jni_sig!(() -> java.lang.String), &[])?
        .l()?;
    let jstr: JString = unsafe { JString::from_raw(env, result.as_raw()) };
    Ok(jstr.try_to_string(env)?)
}
