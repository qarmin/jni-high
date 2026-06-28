package com.example;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;

/**
 * Java helper for file/folder picking and URL opening.
 *
 * Registered as native methods by jni-high at startup; Rust holds a reference to this class
 * via a cached JClass. Static methods receive the Activity as their first argument when
 * the bridge method is declared WITHOUT #[no_activity].
 *
 * Build and convert to DEX:
 *   javac -cp $ANDROID_SDK/platforms/android-34/android.jar CediniaFilePicker.java
 *   d8 --output out/ CediniaFilePicker.class
 *   # Embed as: dex = include_bytes!(concat!(env!("OUT_DIR"), "/file_picker.dex"))
 */
public class CediniaFilePicker {

    // Rust calls this -> Rust: static fn launch_pick_directory(start_path: &str, is_include: bool)
    // Activity is passed by jni-high as the first argument automatically.
    public static void launchPickDirectory(Activity activity, String startPath, boolean isInclude) {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        if (startPath != null && !startPath.isEmpty()) {
            intent.putExtra("android.provider.extra.INITIAL_URI", Uri.parse(startPath));
        }
        // isInclude flag is passed through a custom extra so Rust can receive it in the callback.
        intent.putExtra("is_include", isInclude);
        activity.startActivityForResult(intent, isInclude ? 100 : 101);
    }

    // Rust calls this -> Rust: static fn open_url(url: &str)
    public static void openUrl(Activity activity, String url) {
        Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
        activity.startActivity(intent);
    }

    // Rust calls this -> Rust: static fn open_file(path: &str)
    public static void openFile(Activity activity, String path) {
        Intent intent = new Intent(Intent.ACTION_VIEW);
        intent.setDataAndType(Uri.parse("file://" + path), "*/*");
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        activity.startActivity(intent);
    }

    // Rust calls this -> Rust: static fn open_folder(path: &str)
    public static void openFolder(Activity activity, String path) {
        Intent intent = new Intent(Intent.ACTION_VIEW);
        intent.setDataAndType(Uri.parse("file://" + path), "resource/folder");
        activity.startActivity(intent);
    }

    // Java calls this -> Rust: callback fn on_directory_picked(path: String, is_include: bool)
    // jni-high registers this as a JNI native method; no Java_* naming needed.
    // Called from onActivityResult when the user picks a directory.
    public static native void onDirectoryPicked(String path, boolean isInclude);
}
