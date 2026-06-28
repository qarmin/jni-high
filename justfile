set export := true

adb          := "adb"
demo_package := "io.github.jni_high.demo"
demo_activity := "android.app.NativeActivity"
demo_apk     := "examples/demo/target/debug/apk/jni_high_demo.apk"
demo_apk_rel := "examples/demo/target/release/apk/jni_high_demo.apk"

# Check the library crates (no Slint, fast)
default: check

check:
    cargo check --workspace

# format + clippy
fix:
    cargo +nightly fmt
    cargo clippy --fix --allow-dirty --allow-staged --all-features --all-targets
    cargo +nightly fmt
    cargo fmt

fixn:
    cargo +nightly fmt
    cargo +nightly clippy --fix --allow-dirty --allow-staged --all-features --all-targets
    cargo +nightly fmt
    cargo fmt

# Check the demo crate on desktop (includes Slint compilation)
check-demo:
    cargo check --manifest-path examples/demo/Cargo.toml

# Run the demo on desktop
run:
    cargo run --manifest-path examples/demo/Cargo.toml --bin jni-high-demo

# Debug build + install + launch on connected Android device
demo:
    #!/usr/bin/env bash
    set -euo pipefail
    cd examples/demo
    cargo apk build --lib
    cd ../..
    {{adb}} install -r {{demo_apk}}
    {{adb}} shell am start -n {{demo_package}}/{{demo_activity}}
    {{adb}} logcat -v time "jni-high-demo:V" "RustStdoutStderr:V" "*:S"

# Release build + install + launch
demor:
    #!/usr/bin/env bash
    set -euo pipefail
    cd examples/demo
    cargo apk build --lib --release
    cd ../..
    {{adb}} install -r {{demo_apk_rel}}
    {{adb}} shell am start -n {{demo_package}}/{{demo_activity}}

# Logcat for the normal demo (app logs only)
log:
    {{adb}} logcat -v time "jni-high-demo:V" "RustStdoutStderr:V" "*:S"

# Logcat for the ASAN demo build - includes crash daemon, ASAN reports, app logs.
# Run this in a separate terminal while the app is running.
log_asan:
    {{adb}} logcat -v time "jni-high-demo:V" "RustStdoutStderr:V" "DEBUG:V" "libc:V" "crash_dump:V" "tombstoned:V" "ActivityManager:I" "*:S"

# Build, install, and launch a debug APK instrumented with AddressSanitizer.
#
# ASAN is injected via wrap.sh + LD_PRELOAD - the only supported injection method
# on Android. Requires extractNativeLibs=true (set in Cargo.toml so cargo-apk
# emits it) and a debuggable build so Android honours wrap.sh.
#
# Pass `smoke` to trigger a deliberate heap-buffer-overflow at startup to
# verify ASan is active before running real code:
#   just demo_asan smoke
#
# Requires:
#   ANDROID_NDK_HOME - path to NDK root (e.g. $ANDROID_HOME/ndk/26.3.11579264)
#   ANDROID_HOME     - path to Android SDK root
#   adb in PATH, debug keystore at ~/.android/debug.keystore
demo_asan smoke="":
    #!/usr/bin/env bash
    set -euo pipefail
    : "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to your NDK root (e.g. \$ANDROID_HOME/ndk/26.3.11579264)}"
    : "${ANDROID_HOME:?Set ANDROID_HOME to your Android SDK root}"
    ROOT="$(pwd)"
    TRIPLE=aarch64-linux-android
    API=23
    ABI=arm64-v8a

    HOSTTAG=$(ls "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt" | head -n1)
    TOOLBIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOSTTAG/bin"

    # 1. Build the standard APK. Cargo.toml sets extract_native_libs=true so
    #    cargo-apk emits android:extractNativeLibs="true" in the manifest.
    #    That attribute lets Android extract libs to disk so wrap.sh can run.
    cd examples/demo && cargo apk build --lib && cd "$ROOT"
    APK="$ROOT/{{demo_apk}}"

    # 2. Locate the ASAN runtime shipped with the NDK.
    ASAN_RT=$(find "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOSTTAG/lib/clang" \
        -name "libclang_rt.asan-aarch64-android.so" | head -n1)
    [ -n "$ASAN_RT" ] || { echo "ASan runtime not found in NDK at $ANDROID_NDK_HOME"; exit 1; }

    # 3. Rebuild the .so with ASan instrumentation via nightly.
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TOOLBIN/${TRIPLE}${API}-clang"
    export CC_aarch64_linux_android="$TOOLBIN/${TRIPLE}${API}-clang"
    export AR_aarch64_linux_android="$TOOLBIN/llvm-ar"
    export RUSTFLAGS="-Zsanitizer=address -Cforce-frame-pointers=yes -Cdebuginfo=2"
    cargo +nightly build --manifest-path examples/demo/Cargo.toml --target "$TRIPLE" --lib
    ASAN_SO="$ROOT/examples/demo/target/$TRIPLE/debug/libjni_high_demo.so"

    # 4. Stage instrumented .so, ASAN runtime, and wrap.sh into a temp tree;
    #    inject all three into the APK under lib/arm64-v8a/.
    TMP=$(mktemp -d)
    trap "rm -rf '$TMP'" EXIT
    mkdir -p "$TMP/lib/$ABI"

    cp "$ASAN_SO" "$TMP/lib/$ABI/"
    cp "$ASAN_RT" "$TMP/lib/$ABI/"
    cp examples/demo/asan_wrap.sh "$TMP/lib/$ABI/wrap.sh"

    cd "$TMP" && zip -r "$APK" lib/ && cd "$ROOT"

    # 5. Re-sign the modified APK with the debug keystore.
    APKSIGNER=$(find "$ANDROID_HOME/build-tools" -name "apksigner" | sort -V | tail -1)
    [ -n "$APKSIGNER" ] || { echo "apksigner not found in $ANDROID_HOME/build-tools"; exit 1; }
    "$APKSIGNER" sign --ks ~/.android/debug.keystore --ks-pass pass:android --key-pass pass:android "$APK"

    # 5b. Ensure extractNativeLibs=true is in the manifest (required for wrap.sh).
    #     cargo-apk should emit it from Cargo.toml; patcher is a safety net if not.
    python3 examples/demo/patch_manifest.py "$APK"

    # 6. Install, optionally arm the smoke test, clear logcat, launch.
    # --no-incremental: Incremental Install streams libs on-demand so they are
    # never extracted to disk; wrap.sh would never execute and ASAN can't load.
    {{adb}} install --no-incremental -r "$APK"
    if [ "{{smoke}}" = "smoke" ]; then
        {{adb}} shell "touch /data/local/tmp/jni_high_asan_smoketest"
        echo "Smoke test enabled: app will deliberately heap-overflow at startup."
    fi
    {{adb}} logcat -c || true
    {{adb}} shell am start -n {{demo_package}}/{{demo_activity}}
    echo ""
    echo "App launched. Run 'just log_asan' in another terminal to see logs."
    echo "Or run 'just demo_symbolize' after a crash to symbolize the stack."
    # Wait a moment then dump the tombstone; the crash typically happens within seconds.
    sleep 5
    echo ""
    echo "--- tombstone (most recent crash, if any) ---"
    {{adb}} shell "ls -t /data/tombstones/ 2>/dev/null | head -1 | xargs -I{} cat /data/tombstones/{} 2>/dev/null" || true

upgrade:
    cargo +nightly -Z unstable-options update --breaking
    cargo update

# init = one-time bootstrap of a fresh checkout; sync = re-sync after pulling.
init:
    cargo fetch

sync:
    cargo fetch

setup_sanitizer:
    rustup install nightly
    rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu
    rustup component add llvm-tools-preview --toolchain nightly-x86_64-unknown-linux-gnu

# Symbolize the most recent tombstone from the device.
# Pass an optional path to a saved tombstone file instead.
demo_symbolize file="":
    #!/usr/bin/env bash
    set -euo pipefail
    : "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to your NDK root}"
    SYM="examples/demo/target/aarch64-linux-android/debug"
    if [ -n "{{file}}" ]; then
        "$ANDROID_NDK_HOME/ndk-stack" -sym "$SYM" -dump "{{file}}"
    else
        TOMB=$({{adb}} shell "ls -t /data/tombstones/ 2>/dev/null | head -1" | tr -d '\r')
        [ -n "$TOMB" ] || { echo "No tombstones found on device"; exit 1; }
        echo "Symbolizing /data/tombstones/$TOMB ..."
        {{adb}} shell cat "/data/tombstones/$TOMB" | "$ANDROID_NDK_HOME/ndk-stack" -sym "$SYM"
    fi
