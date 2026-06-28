use jni::objects::{JObject, JValue};
use jni::signature::RuntimeMethodSignature;
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

// Common Android permission strings for convenience.
pub const POST_NOTIFICATIONS: &str = "android.permission.POST_NOTIFICATIONS";
pub const READ_EXTERNAL_STORAGE: &str = "android.permission.READ_EXTERNAL_STORAGE";
pub const WRITE_EXTERNAL_STORAGE: &str = "android.permission.WRITE_EXTERNAL_STORAGE";
pub const MANAGE_EXTERNAL_STORAGE: &str = "android.permission.MANAGE_EXTERNAL_STORAGE";
// Granular media permissions replacing READ_EXTERNAL_STORAGE on API 33+.
pub const READ_MEDIA_IMAGES: &str = "android.permission.READ_MEDIA_IMAGES";
pub const READ_MEDIA_VIDEO: &str = "android.permission.READ_MEDIA_VIDEO";
pub const READ_MEDIA_AUDIO: &str = "android.permission.READ_MEDIA_AUDIO";
pub const CAMERA: &str = "android.permission.CAMERA";
pub const RECORD_AUDIO: &str = "android.permission.RECORD_AUDIO";
pub const ACCESS_FINE_LOCATION: &str = "android.permission.ACCESS_FINE_LOCATION";
pub const ACCESS_COARSE_LOCATION: &str = "android.permission.ACCESS_COARSE_LOCATION";
pub const ACCESS_NETWORK_STATE: &str = "android.permission.ACCESS_NETWORK_STATE";

/// Returns `true` when the given permission has been granted by the user.
///
/// Uses `Activity.checkSelfPermission(permission)` and compares against
/// `PackageManager.PERMISSION_GRANTED` (== 0).
pub fn is_granted(permission: &str) -> BridgeResult<bool> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let j_perm: JObject = env.new_string(permission)?.into();
        let result: i32 = env
            .call_method(
                activity,
                jni_str!("checkSelfPermission"),
                jni_sig!((permission: java.lang.String) -> int),
                &[JValue::Object(&j_perm)],
            )?
            .i()?;
        // PackageManager.PERMISSION_GRANTED == 0
        Ok(result == 0)
    })
}

/// Ask the user to grant one or more dangerous permissions via the system dialog.
///
/// This is a fire-and-forget call: it opens the system permission dialog and returns
/// immediately. The result is delivered via `Activity.onRequestPermissionsResult`,
/// which NativeActivity does not expose directly - check `is_granted()` after the user
/// interacts with the dialog.
///
/// `request_code` is passed back through `onRequestPermissionsResult` and can be any
/// non-negative integer chosen by the caller.
///
/// Note: normal permissions (e.g. `VIBRATE`, `ACCESS_NETWORK_STATE`) are auto-granted at
/// install time and do not need to be requested here.
pub fn request(permissions: &[&str], request_code: i32) -> BridgeResult<()> {
    if permissions.is_empty() {
        return Ok(());
    }
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    let owned: Vec<String> = permissions.iter().map(|s| s.to_string()).collect();
    ctx.attach(|env, activity| {
        let string_class = env.find_class(jni_str!("java/lang/String"))?;
        let null_obj = JObject::null();
        let array = env.new_object_array(owned.len() as i32, &string_class, &null_obj)?;
        for (i, perm) in owned.iter().enumerate() {
            let j_perm: JObject = env.new_string(perm.as_str())?.into();
            array.set_element(env, i, &j_perm)?;
        }
        let sig = RuntimeMethodSignature::from_str("([Ljava/lang/String;I)V").expect("valid JNI signature");
        env.call_method(
            activity,
            jni_str!("requestPermissions"),
            &sig.method_signature(),
            &[JValue::Object(array.as_ref()), JValue::Int(request_code)],
        )?;
        Ok(())
    })
}
