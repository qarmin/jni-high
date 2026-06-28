use jni::jni_str;
use jni::objects::{JObject, JString};
use jni::signature::{FieldSignature, RuntimeFieldSignature};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// Basic device and OS information read from `android.os.Build`.
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub android_version: String,
    pub sdk_int: i32,
}

/// Read device info from `android.os.Build` static fields.
///
/// All fields are read in a single JNI attach - no Activity reference is required.
pub fn info() -> BridgeResult<DeviceInfo> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, _activity| {
        let rfs_str = RuntimeFieldSignature::from_str("Ljava/lang/String;").expect("valid");
        let str_sig = FieldSignature::from(&rfs_str);
        let rfs_int = RuntimeFieldSignature::from_str("I").expect("valid");
        let int_sig = FieldSignature::from(&rfs_int);

        let manufacturer = read_string(env, jni_str!("android/os/Build"), jni_str!("MANUFACTURER"), &str_sig)?;
        let model = read_string(env, jni_str!("android/os/Build"), jni_str!("MODEL"), &str_sig)?;
        let android_version = read_string(env, jni_str!("android/os/Build$VERSION"), jni_str!("RELEASE"), &str_sig)?;
        let sdk_int: i32 = env
            .get_static_field(jni_str!("android/os/Build$VERSION"), jni_str!("SDK_INT"), &int_sig)?
            .i()?;

        Ok(DeviceInfo {
            manufacturer,
            model,
            android_version,
            sdk_int,
        })
    })
}

fn read_string(
    env: &mut jni::Env<'_>,
    class: &jni::strings::JNIStr,
    field: &jni::strings::JNIStr,
    sig: &FieldSignature,
) -> BridgeResult<String> {
    let obj: JObject = env.get_static_field(class, field, sig)?.l()?;
    // Safety: the field is declared as java.lang.String in the Android SDK.
    let jstr: JString = unsafe { JString::from_raw(env, obj.as_raw()) };
    Ok(jstr.try_to_string(env)?)
}
