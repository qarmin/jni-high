package com.example;

import android.os.Build;

/**
 * Device information helpers.
 *
 * All methods are #[no_activity] - they only read static Build fields.
 * No parameters are needed on the Java side.
 *
 * Compiled into the same DEX as AppLocale (locale.dex) - multiple classes can share one DEX.
 */
public class AppDeviceInfo {

    // -> Rust: #[no_activity] static fn sdk_version() -> i32
    public static int sdkVersion() {
        return Build.VERSION.SDK_INT;
    }

    // -> Rust: #[no_activity] static fn model() -> String
    public static String model() {
        return Build.MANUFACTURER + " " + Build.MODEL;
    }
}
