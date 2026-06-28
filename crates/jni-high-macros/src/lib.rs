use proc_macro::TokenStream;
use syn::parse_macro_input;

mod codegen;
mod parse;
mod types;

/// Generate high-level Rust wrappers for a Java/Kotlin helper class loaded from a DEX blob.
///
/// # Syntax
///
/// ```ignore
/// android_bridge! {
///     dex = include_bytes!("my_helpers.dex"),
///
///     class MyHelper {
///         java_name = "com.example.MyHelper",
///
///         // Call a Java static method (activity is passed automatically).
///         static fn get_locale() -> String;
///
///         // Call without activity (for pure-logic helpers).
///         #[no_activity]
///         static fn add(a: i32, b: i32) -> i32;
///
///         // Java calls back into Rust via a registered native method.
///         callback fn on_result(value: String);
///     }
/// }
/// ```
#[proc_macro]
pub fn android_bridge(input: TokenStream) -> TokenStream {
    let block = parse_macro_input!(input as parse::BridgeBlock);
    match codegen::generate(&block) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
