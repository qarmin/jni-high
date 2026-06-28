package com.example;

import android.app.Activity;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.provider.Settings;

import androidx.core.app.NotificationCompat;
import androidx.core.app.NotificationManagerCompat;

/**
 * Notifications helper.
 *
 * Methods marked #[no_activity] in the bridge receive the application Context as their first
 * argument instead of Activity. Pass the application context via a stored reference or obtain
 * it from the activity stored at startup.
 *
 * Build and convert to DEX:
 *   javac -cp $ANDROID_SDK/platforms/android-34/android.jar:$APPCOMPAT_JAR AppNotifications.java
 *   d8 --output out/ AppNotifications.class
 *   # Embed as: dex = include_bytes!(concat!(env!("OUT_DIR"), "/notifications.dex"))
 */
public class AppNotifications {

    private static final String CHANNEL_ID = "czkawka_main";
    private static Context appContext;

    // Called once at startup by jni-high when the first method requiring context runs.
    // In practice, store the application context during Activity.onCreate.
    public static void init(Context context) {
        appContext = context.getApplicationContext();
        createNotificationChannel();
    }

    // -> Rust: #[no_activity] static fn are_enabled() -> bool
    //
    // #[no_activity] means jni-high does NOT inject Activity as the first Java argument.
    // The Java method signature here takes no parameters.
    public static boolean areEnabled() {
        if (appContext == null) return false;
        return NotificationManagerCompat.from(appContext).areNotificationsEnabled();
    }

    // -> Rust: static fn open_settings()
    //
    // Without #[no_activity] the Activity is the first argument.
    public static void openSettings(Activity activity) {
        Intent intent = new Intent();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            intent.setAction(Settings.ACTION_APP_NOTIFICATION_SETTINGS);
            intent.putExtra(Settings.EXTRA_APP_PACKAGE, activity.getPackageName());
        } else {
            intent.setAction("android.settings.APP_NOTIFICATION_SETTINGS");
            intent.putExtra("app_package", activity.getPackageName());
            intent.putExtra("app_uid", activity.getApplicationInfo().uid);
        }
        activity.startActivity(intent);
    }

    // -> Rust: static fn send_scan_completed(file_count: i32, in_background: bool)
    public static void sendScanCompleted(Activity activity, int fileCount, boolean inBackground) {
        if (appContext == null) return;

        String title = "Scan complete";
        String body = "Found " + fileCount + " files" + (inBackground ? " (background)" : "");

        NotificationCompat.Builder builder = new NotificationCompat.Builder(appContext, CHANNEL_ID)
                .setSmallIcon(android.R.drawable.ic_popup_disk_full)
                .setContentTitle(title)
                .setContentText(body)
                .setPriority(NotificationCompat.PRIORITY_DEFAULT)
                .setAutoCancel(true);

        NotificationManagerCompat.from(appContext).notify(1, builder.build());
    }

    private static void createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && appContext != null) {
            NotificationChannel channel = new NotificationChannel(
                    CHANNEL_ID, "Scan results", NotificationManager.IMPORTANCE_DEFAULT);
            NotificationManager nm = appContext.getSystemService(NotificationManager.class);
            nm.createNotificationChannel(channel);
        }
    }
}
