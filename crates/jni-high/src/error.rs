/// Opaque wrapper around `jni::errors::Error` - users do not need `jni` as a direct dep.
#[derive(Debug)]
pub struct JniError(pub(crate) jni::errors::Error);

impl JniError {
    pub fn into_inner(self) -> jni::errors::Error {
        self.0
    }
}

impl std::fmt::Display for JniError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug)]
pub enum BridgeError {
    /// `AndroidContext::init` was never called.
    ContextNotInitialized,
    /// `vm_as_ptr()` returned null - app is paused/stopped; JNI calls are unsafe here.
    VmNotAvailable,
    /// The Java class was not found in the DEX or system classpath.
    ClassNotFound(String),
    /// JNI returned an error.
    Jni(JniError),
    /// A Java exception was thrown and captured.
    JavaException { class: String, message: String },
    /// A Rust panic occurred inside a JNI callback.
    CallbackPanic,
    /// Type conversion failed (e.g. null where non-null was expected).
    NullPointer { context: &'static str },
    /// An I/O error (e.g. writing the DEX temp file for the DexClassLoader fallback).
    Io(std::io::Error),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContextNotInitialized => write!(f, "AndroidContext not initialized"),
            Self::VmNotAvailable => write!(f, "JVM not available (app paused?)"),
            Self::ClassNotFound(name) => write!(f, "Java class not found: {name}"),
            Self::Jni(e) => write!(f, "JNI error: {e}"),
            Self::JavaException { class, message } => {
                write!(f, "Java exception {class}: {message}")
            }
            Self::CallbackPanic => write!(f, "Rust panic in JNI callback"),
            Self::NullPointer { context } => write!(f, "Null pointer in {context}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<jni::errors::Error> for BridgeError {
    fn from(e: jni::errors::Error) -> Self {
        Self::Jni(JniError(e))
    }
}

pub type BridgeResult<T> = Result<T, BridgeError>;

pub trait BridgeResultExt {
    /// Log an error, silently ignoring `VmNotAvailable` (app is backgrounded).
    fn log_err(self, tag: &str);
}

impl<T> BridgeResultExt for BridgeResult<T> {
    fn log_err(self, tag: &str) {
        if matches!(&self, Err(BridgeError::VmNotAvailable)) {
            return;
        }
        if let Err(e) = self {
            log::error!("{tag}: {e:?}");
        }
    }
}
