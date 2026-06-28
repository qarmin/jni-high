use jni::objects::{JObject, JString, JValue};
use jni::signature::{FieldSignature, RuntimeFieldSignature};
use jni::{jni_sig, jni_str};

use crate::AndroidContext;
use crate::error::{BridgeError, BridgeResult};

/// App-private filesystem paths. Filled in before `AndroidContext::init` is called.
pub struct AppDirs {
    /// Equivalent of `Context.getFilesDir().getAbsolutePath()`.
    pub files_dir: String,
    /// Equivalent of `Context.getCacheDir().getAbsolutePath()`.
    pub cache_dir: String,
}

/// Retrieve the app-private `files/` and `cache/` directory paths.
///
/// This function works **before** `AndroidContext::init` is called because it
/// creates its own temporary JVM attachment internally.
pub fn app_dirs(app: &android_activity::AndroidApp) -> BridgeResult<AppDirs> {
    let vm_ptr = app.vm_as_ptr();
    if vm_ptr.is_null() {
        return Err(BridgeError::VmNotAvailable);
    }
    // Safety: vm_as_ptr() from android-activity is a valid JavaVM pointer when non-null.
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr as *mut _) };
    let activity_ptr = app.activity_as_ptr();
    vm.attach_current_thread(|env| -> jni::errors::Result<AppDirs> {
        // Safety: activity_as_ptr() is the raw jobject for the current Activity.
        let activity = unsafe { JObject::from_raw(&*env, activity_ptr as *mut _) };
        let files_dir = dir_absolute_path(env, &activity, jni_str!("getFilesDir"))?;
        let cache_dir = dir_absolute_path(env, &activity, jni_str!("getCacheDir"))?;
        Ok(AppDirs { files_dir, cache_dir })
    })
    .map_err(BridgeError::from)
}

fn dir_absolute_path(
    env: &mut jni::Env<'_>,
    activity: &JObject<'_>,
    method: &jni::strings::JNIStr,
) -> jni::errors::Result<String> {
    let dir_obj: JObject = env
        .call_method(activity, method, jni_sig!(() -> java.io.File), &[])?
        .l()?;
    let path_obj: JObject = env
        .call_method(
            &dir_obj,
            jni_str!("getAbsolutePath"),
            jni_sig!(() -> java.lang.String),
            &[],
        )?
        .l()?;
    // Safety: path_obj is a java.lang.String returned by getAbsolutePath.
    let jstr: JString = unsafe { JString::from_raw(env, path_obj.as_raw()) };
    jstr.try_to_string(env)
}

/// Returns `true` when the calling process is in the foreground
/// (`ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND == 100`).
///
/// Returns `false` on any JNI error or when the context is not initialized.
pub fn is_in_foreground() -> BridgeResult<bool> {
    let ctx = AndroidContext::get().ok_or(BridgeError::ContextNotInitialized)?;
    ctx.attach(|env, activity| {
        let am = get_activity_manager(env, activity)?;
        if am.is_null() {
            return Ok(false);
        }
        let processes: JObject = env
            .call_method(
                &am,
                jni_str!("getRunningAppProcesses"),
                jni_sig!(() -> java.util.List),
                &[],
            )?
            .l()?;
        if processes.is_null() {
            return Ok(false);
        }
        find_foreground_status(env, &processes)
    })
}

fn get_activity_manager<'env>(env: &mut jni::Env<'env>, activity: &JObject<'_>) -> BridgeResult<JObject<'env>> {
    let svc_name: JObject = env.new_string("activity")?.into();
    Ok(env
        .call_method(
            activity,
            jni_str!("getSystemService"),
            jni_sig!((name: java.lang.String) -> java.lang.Object),
            &[JValue::Object(&svc_name)],
        )?
        .l()?)
}

fn find_foreground_status(env: &mut jni::Env<'_>, processes: &JObject<'_>) -> BridgeResult<bool> {
    let my_pid = std::process::id() as i32;
    let count: i32 = env
        .call_method(processes, jni_str!("size"), jni_sig!(() -> int), &[])?
        .i()?;
    // "I" is always a valid JNI type descriptor.
    let int_rfs = RuntimeFieldSignature::from_str("I").expect("valid JNI primitive descriptor");
    let int_sig = FieldSignature::from(&int_rfs);
    for i in 0..count {
        let item: JObject = env
            .call_method(
                processes,
                jni_str!("get"),
                jni_sig!((index: int) -> java.lang.Object),
                &[JValue::Int(i)],
            )?
            .l()?;
        let pid: i32 = env.get_field(&item, jni_str!("pid"), &int_sig)?.i()?;
        if pid == my_pid {
            let importance: i32 = env.get_field(&item, jni_str!("importance"), &int_sig)?.i()?;
            // IMPORTANCE_FOREGROUND = 100
            return Ok(importance == 100);
        }
    }
    Ok(false)
}
