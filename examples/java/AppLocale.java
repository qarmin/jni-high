package com.example;

import android.text.format.DateFormat;
import android.content.Context;

import java.util.Locale;

/**
 * Locale and time-format helpers.
 *
 * All methods are #[no_activity] in the bridge - they only need a Locale, not the Activity.
 * The Java side therefore takes no arguments at all.
 *
 * Build and convert to DEX:
 *   javac -cp $ANDROID_SDK/platforms/android-34/android.jar AppLocale.java
 *   d8 --output out/ locale.dex  AppLocale.class AppDeviceInfo.class
 *   # Embed as: dex = include_bytes!(concat!(env!("OUT_DIR"), "/locale.dex"))
 */
public class AppLocale {

    // -> Rust: #[no_activity] static fn language_tag() -> String
    public static String languageTag() {
        return Locale.getDefault().toLanguageTag();
    }

    // -> Rust: #[no_activity] static fn country_code() -> Option<String>
    //
    // Returns null when no country is set - jni-high maps null String -> None in Rust.
    public static String countryCode() {
        String country = Locale.getDefault().getCountry();
        return country.isEmpty() ? null : country;
    }

    // -> Rust: #[no_activity] static fn preferred_hour_format() -> i32
    //
    // Returns 24 or 12 depending on system locale/user preference.
    // DateFormat.is24HourFormat requires a Context; store it at startup if needed,
    // or fall back to locale-based detection as shown here.
    public static int preferredHourFormat() {
        // Use the locale's preferred format as a heuristic when no Context is available.
        // For a more accurate result, call DateFormat.is24HourFormat(context) in init().
        String pattern = android.text.format.DateFormat.getBestDateTimePattern(
                Locale.getDefault(), "Hm");
        return pattern.contains("H") ? 24 : 12;
    }
}
