use std::path::Path;

/// Compile a `.java` or `.kt` source file into a DEX blob and write it to `out_path`.
///
/// This is a stub. A real implementation would shell out to `javac`/`kotlinc` + `d8`.
/// For now, downstream crates are expected to supply pre-compiled DEX files via
/// `include_bytes!` and pass them as the `dex =` argument in `android_bridge!`.
///
/// # Errors
///
/// Always returns an error (not yet implemented).
pub fn compile_dex(_source: &Path, _out_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    Err("jni-high-build: compile_dex is not yet implemented; supply a pre-compiled .dex file".into())
}
