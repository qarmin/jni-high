use std::sync::OnceLock;

use jni::JavaVM;
use jni::objects::{Global, JClass, JObject};

use crate::error::{BridgeError, BridgeResult};

pub struct AndroidContext {
    /// `AndroidApp` is `Send + Sync`; `activity_as_ptr()` is called fresh on every attach.
    app: android_activity::AndroidApp,
    /// The JVM - stable for the process lifetime once init() is called.
    vm: JavaVM,
}

impl AndroidContext {
    /// Initialize the singleton. Must be called once at app startup.
    /// Subsequent calls are silently ignored.
    pub fn init(app: android_activity::AndroidApp) {
        let vm_ptr = app.vm_as_ptr();
        assert!(
            !vm_ptr.is_null(),
            "jni-high: AndroidApp has a null JVM pointer at init time"
        );
        // Safety: ptr comes from android-activity which guarantees it is a valid JavaVM.
        let vm = unsafe { JavaVM::from_raw(vm_ptr as *mut _) };
        let _ = CONTEXT.set(AndroidContext { app, vm });
    }

    /// Get the singleton, or `None` if `init` was never called.
    pub fn get() -> Option<&'static Self> {
        CONTEXT.get()
    }

    /// Attach the current thread to the JVM and run a closure with a JNI env and
    /// a fresh `Activity` reference.
    ///
    /// Returns `BridgeError::VmNotAvailable` when the JVM pointer is null (app paused).
    pub fn attach<F, T>(&self, f: F) -> BridgeResult<T>
    where
        F: FnOnce(&mut jni::Env<'_>, &JObject<'_>) -> BridgeResult<T>,
    {
        if self.app.vm_as_ptr().is_null() {
            return Err(BridgeError::VmNotAvailable);
        }
        let activity_ptr = self.app.activity_as_ptr();
        self.vm.attach_current_thread(|env| {
            // Safety: activity_as_ptr() is the raw JNI jobject for the current Activity.
            let activity = unsafe { JObject::from_raw(&*env, activity_ptr as *mut _) };
            f(env, &activity)
        })
    }

    /// Load a DEX and return a `Global` reference to the class loader.
    /// The global ref lives for the process lifetime when stored in a `OnceLock`.
    pub fn load_dex(&self, dex_data: &[u8]) -> BridgeResult<Global<JObject<'static>>> {
        self.attach(|env, activity| {
            let loader = crate::dex::load_dex(env, activity, dex_data)?;
            Ok(env.new_global_ref(loader)?)
        })
    }

    /// Find a class inside a DEX loader by slash-notation name and return a `Global` class ref.
    pub fn find_class_in_loader(
        &self,
        loader: &Global<JObject<'static>>,
        slash_name: &str,
    ) -> BridgeResult<Global<JClass<'static>>> {
        self.attach(|env, _activity| {
            let class = crate::dex::find_class_in_loader(env, loader.as_obj(), slash_name)?;
            Ok(env.new_global_ref(class)?)
        })
    }
}

static CONTEXT: OnceLock<AndroidContext> = OnceLock::new();
