use jni::Env;
use jni::objects::{JClass, JObject, JString, JValue};

use crate::error::{BridgeError, BridgeResult};

/// Load a DEX into an `InMemoryDexClassLoader` (API 26+), falling back to
/// `DexClassLoader` (writes a temp file) when the in-memory loader is unavailable.
///
/// Returns a local reference to the class loader object. Callers must turn it
/// into a `Global` ref before the JNI local frame is released.
pub fn load_dex<'env>(env: &mut Env<'env>, activity: &JObject<'_>, dex_data: &[u8]) -> BridgeResult<JObject<'env>> {
    let dex_buffer = unsafe { env.new_direct_byte_buffer(dex_data.as_ptr() as *mut _, dex_data.len()) }?;
    let parent_loader = env
        .call_method(
            activity,
            unsafe { jni::strings::JNIStr::from_cstr_unchecked(c"getClassLoader") },
            unsafe {
                jni::signature::MethodSignature::from_raw_parts(
                    jni::strings::JNIStr::from_cstr_unchecked(c"()Ljava/lang/ClassLoader;"),
                    &[],
                    jni::signature::JavaType::Object,
                )
            },
            &[],
        )?
        .l()?;

    match env.new_object(
        unsafe { jni::strings::JNIStr::from_cstr_unchecked(c"dalvik/system/InMemoryDexClassLoader") },
        unsafe {
            jni::signature::MethodSignature::from_raw_parts(
                jni::strings::JNIStr::from_cstr_unchecked(c"(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V"),
                &[jni::signature::JavaType::Object, jni::signature::JavaType::Object],
                jni::signature::JavaType::Primitive(jni::signature::Primitive::Void),
            )
        },
        &[JValue::Object(&dex_buffer), JValue::Object(&parent_loader)],
    ) {
        Ok(loader) => {
            log::debug!("jni-high: DEX loaded via InMemoryDexClassLoader");
            Ok(loader)
        }
        Err(e) => {
            log::debug!("jni-high: InMemoryDexClassLoader failed ({e:?}), trying DexClassLoader");
            let _ = env.exception_clear();
            fallback_dex_loader(env, activity, dex_data, &parent_loader)
        }
    }
}

fn fallback_dex_loader<'env>(
    env: &mut Env<'env>,
    activity: &JObject<'_>,
    dex_data: &[u8],
    parent_loader: &JObject<'_>,
) -> BridgeResult<JObject<'env>> {
    let cache_dir_obj = env
        .call_method(
            activity,
            unsafe { jni::strings::JNIStr::from_cstr_unchecked(c"getCodeCacheDir") },
            unsafe {
                jni::signature::MethodSignature::from_raw_parts(
                    jni::strings::JNIStr::from_cstr_unchecked(c"()Ljava/io/File;"),
                    &[],
                    jni::signature::JavaType::Object,
                )
            },
            &[],
        )?
        .l()?;
    let path_obj = env
        .call_method(
            &cache_dir_obj,
            unsafe { jni::strings::JNIStr::from_cstr_unchecked(c"getAbsolutePath") },
            unsafe {
                jni::signature::MethodSignature::from_raw_parts(
                    jni::strings::JNIStr::from_cstr_unchecked(c"()Ljava/lang/String;"),
                    &[],
                    jni::signature::JavaType::Object,
                )
            },
            &[],
        )?
        .l()?;
    let j_path_str: JString = unsafe { JString::from_raw(env, path_obj.as_raw()) };
    let path_str = j_path_str.try_to_string(env).map_err(BridgeError::from)?;
    let dex_path = format!("{path_str}/jni_high_bridge.dex");
    let oats_path = format!("{path_str}/oats");
    std::fs::write(&dex_path, dex_data).map_err(BridgeError::Io)?;
    let _ = std::fs::create_dir(&oats_path);
    let j_dex = env.new_string(&dex_path)?;
    let j_oats = env.new_string(&oats_path)?;
    let loader = env.new_object(
        unsafe { jni::strings::JNIStr::from_cstr_unchecked(c"dalvik/system/DexClassLoader") },
        unsafe {
            jni::signature::MethodSignature::from_raw_parts(
                jni::strings::JNIStr::from_cstr_unchecked(
                    c"(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/ClassLoader;)V",
                ),
                &[
                    jni::signature::JavaType::Object,
                    jni::signature::JavaType::Object,
                    jni::signature::JavaType::Object,
                    jni::signature::JavaType::Object,
                ],
                jni::signature::JavaType::Primitive(jni::signature::Primitive::Void),
            )
        },
        &[
            JValue::Object(&j_dex),
            JValue::Object(&j_oats),
            JValue::Object(&JObject::null()),
            JValue::Object(parent_loader),
        ],
    )?;
    log::debug!("jni-high: DEX loaded via DexClassLoader");
    Ok(loader)
}

/// Find a class inside a previously loaded DEX class loader object.
/// `class_name` must use dot notation (e.g. `"com.example.MyHelper"`), as
/// expected by `ClassLoader.findClass()`.
/// Returns a local `JClass` reference valid for the current JNI frame.
pub fn find_class_in_loader<'env>(
    env: &mut Env<'env>,
    loader: &JObject<'_>,
    class_name: &str,
) -> BridgeResult<JClass<'env>> {
    let class_name_jstr = env.new_string(class_name)?;
    let class_obj = env
        .call_method(
            loader,
            unsafe { jni::strings::JNIStr::from_cstr_unchecked(c"findClass") },
            unsafe {
                jni::signature::MethodSignature::from_raw_parts(
                    jni::strings::JNIStr::from_cstr_unchecked(c"(Ljava/lang/String;)Ljava/lang/Class;"),
                    &[jni::signature::JavaType::Object],
                    jni::signature::JavaType::Object,
                )
            },
            &[JValue::Object(&class_name_jstr)],
        )?
        .l()?;
    Ok(unsafe { JClass::from_raw(env, class_obj.as_raw()) })
}
