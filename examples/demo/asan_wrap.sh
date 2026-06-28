#!/system/bin/sh
# AddressSanitizer launcher for jni-high demo debug builds.
#
# Android injects an ASan runtime through a wrap.sh placed in the APK at
# lib/<abi>/wrap.sh. Requirements: debuggable build + extractNativeLibs=true
# so that $HERE below resolves to a real directory holding the .so files.
#
# Staged by the `demo_asan` just recipe at build time. Do NOT commit this
# under release sources - it would break release builds that ship no ASan
# runtime for the LD_PRELOAD glob to find.

HERE="$(cd "$(dirname "$0")" && pwd)"

# Confirm wrap.sh ran - visible in logcat immediately, before any Rust code.
log -t jni-high-demo -p I "wrap.sh: ASAN injection starting, HERE=$HERE"

# log_to_syslog=true routes ASAN reports through __android_log_write so they
# appear in logcat under the "DEBUG" tag; without this, ASAN writes to stderr
# which is silently discarded on Android.
# detect_leaks is off: LSan is noisy and slow under a GUI/JNI process.
# allow_user_segv_handler keeps Slint signal handling working.
export ASAN_OPTIONS=log_to_syslog=true,allow_user_segv_handler=1,detect_leaks=0,abort_on_error=1

ASAN_LIB=$(ls "$HERE"/libclang_rt.asan-*-android.so)
if [ -f "$HERE/libc++_shared.so" ]; then
    export LD_PRELOAD="$ASAN_LIB $HERE/libc++_shared.so"
else
    export LD_PRELOAD="$ASAN_LIB"
fi

exec "$@"
