pub mod error;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "android")]
pub mod context;
#[cfg(target_os = "android")]
pub mod dex;

#[cfg(target_os = "android")]
pub use context::AndroidContext;
pub use error::{BridgeError, BridgeResult, BridgeResultExt, JniError};
pub use jni_high_macros::android_bridge;

/// Internal module used exclusively by code generated from `android_bridge!`.
/// Not part of the public API - types and paths may change without notice.
#[doc(hidden)]
pub mod __private {
    pub use jni::errors::Error as JniSysError;
    pub use jni::objects::{Global, JClass, JObject, JString, JValue, JValueOwned};
    pub use jni::signature::{JavaType, MethodSignature, Primitive, ReturnType};
    pub use jni::strings::JNIStr;
    pub use jni::sys::{jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort};
    pub use jni::{Env, EnvUnowned, NativeMethod, Outcome};

    #[cfg(target_os = "android")]
    pub use super::dex::{find_class_in_loader, load_dex};
}
