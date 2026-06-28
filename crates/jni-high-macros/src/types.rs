use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

/// Canonical representation of types supported by the bridge.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Char,
    /// `&str` - input-only string
    BorrowedStr,
    /// `String`
    OwnedString,
    /// `Option<String>`
    OptionString,
    /// `()` - void return
    Unit,
}

impl BridgeType {
    /// Parse a `syn::Type` into our simplified type enum.
    pub fn from_syn(ty: &Type) -> syn::Result<Self> {
        let s = quote!(#ty).to_string().replace(' ', "");
        match s.as_str() {
            "bool" => Ok(Self::Bool),
            "i8" => Ok(Self::I8),
            "i16" => Ok(Self::I16),
            "i32" => Ok(Self::I32),
            "i64" => Ok(Self::I64),
            "f32" => Ok(Self::F32),
            "f64" => Ok(Self::F64),
            "char" => Ok(Self::Char),
            "&str" => Ok(Self::BorrowedStr),
            "String" => Ok(Self::OwnedString),
            "Option<String>" => Ok(Self::OptionString),
            "()" => Ok(Self::Unit),
            other => Err(syn::Error::new_spanned(
                ty,
                format!("unsupported bridge type `{other}`"),
            )),
        }
    }

    /// JNI descriptor character(s) for this type.
    pub fn jni_sig_char(&self) -> &'static str {
        match self {
            Self::Bool => "Z",
            Self::I8 => "B",
            Self::I16 => "S",
            Self::I32 => "I",
            Self::I64 => "J",
            Self::F32 => "F",
            Self::F64 => "D",
            Self::Char => "C",
            Self::BorrowedStr | Self::OwnedString | Self::OptionString => "Ljava/lang/String;",
            Self::Unit => "V",
        }
    }

    /// Emit the `::jni_high::__private::JavaType` token for this type (used in MethodSignature args).
    pub fn java_type_token(&self) -> TokenStream {
        match self {
            Self::Bool => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Boolean
            )),
            Self::I8 => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Byte
            )),
            Self::I16 => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Short
            )),
            Self::I32 => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Int
            )),
            Self::I64 => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Long
            )),
            Self::F32 => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Float
            )),
            Self::F64 => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Double
            )),
            Self::Char => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Char
            )),
            Self::BorrowedStr | Self::OwnedString | Self::OptionString => {
                quote!(::jni_high::__private::JavaType::Object)
            }
            Self::Unit => quote!(::jni_high::__private::JavaType::Primitive(
                ::jni_high::__private::Primitive::Void
            )),
        }
    }

    /// Emit the `ReturnType` token for use in `MethodSignature::from_raw_parts`.
    pub fn return_type_token(&self) -> TokenStream {
        self.java_type_token()
    }

    /// Emit the Rust type for the generated method signature.
    pub fn rust_type_token(&self) -> TokenStream {
        match self {
            Self::Bool => quote!(bool),
            Self::I8 => quote!(i8),
            Self::I16 => quote!(i16),
            Self::I32 => quote!(i32),
            Self::I64 => quote!(i64),
            Self::F32 => quote!(f32),
            Self::F64 => quote!(f64),
            Self::Char => quote!(char),
            Self::BorrowedStr => quote!(&str),
            Self::OwnedString => quote!(String),
            Self::OptionString => quote!(Option<String>),
            Self::Unit => quote!(()),
        }
    }

    /// Emit the JNI extern-fn parameter type for callbacks (what Java passes in).
    pub fn callback_param_type(&self) -> TokenStream {
        match self {
            Self::Bool => quote!(::jni_high::__private::jboolean),
            Self::I8 => quote!(::jni_high::__private::jbyte),
            Self::I16 => quote!(::jni_high::__private::jshort),
            Self::I32 => quote!(::jni_high::__private::jint),
            Self::I64 => quote!(::jni_high::__private::jlong),
            Self::F32 => quote!(::jni_high::__private::jfloat),
            Self::F64 => quote!(::jni_high::__private::jdouble),
            Self::Char => quote!(::jni_high::__private::jchar),
            Self::BorrowedStr | Self::OwnedString | Self::OptionString => {
                quote!(::jni_high::__private::JString<'__local>)
            }
            Self::Unit => quote!(()),
        }
    }

    /// The Rust type that a callback handler function receives for this param.
    /// Strings are always delivered as `String` (owned), never `&str`.
    pub fn callback_handler_type(&self) -> TokenStream {
        match self {
            Self::BorrowedStr | Self::OwnedString => quote!(String),
            Self::OptionString => quote!(Option<String>),
            other => other.rust_type_token(),
        }
    }

    /// Emit an expression that converts a raw callback param inside a `with_env_no_catch` closure.
    /// The closure returns `Result<_, jni::errors::Error>`, so strings use `?` for errors.
    /// `param_ident` is the identifier of the raw JNI extern-fn parameter.
    pub fn callback_closure_expr(&self, param_ident: &proc_macro2::Ident) -> TokenStream {
        match self {
            // Cast to u8 first so this compiles when jboolean = bool (Android) or u8 (other).
            Self::Bool => quote!((#param_ident as u8) != 0),
            Self::I8 => quote!(#param_ident as i8),
            Self::I16 => quote!(#param_ident as i16),
            Self::I32 => quote!(#param_ident as i32),
            Self::I64 => quote!(#param_ident as i64),
            Self::F32 => quote!(#param_ident as f32),
            Self::F64 => quote!(#param_ident as f64),
            // jchar is u16; surrogates become REPLACEMENT_CHARACTER.
            Self::Char => {
                quote!(char::from_u32(#param_ident as u32).unwrap_or(char::REPLACEMENT_CHARACTER))
            }
            // All string variants: Java passes JString, we call try_to_string with ?.
            Self::BorrowedStr | Self::OwnedString => quote!(#param_ident.try_to_string(__env)?),
            Self::OptionString => quote! {
                if #param_ident.as_raw().is_null() {
                    None
                } else {
                    Some(#param_ident.try_to_string(__env)?)
                }
            },
            Self::Unit => quote!(()),
        }
    }

    /// Emit code to convert a Rust value to `JValue` for a static method call.
    /// `val_ident` is the Rust variable name of the parameter.
    /// Returns `(preamble, jvalue_expr)` where preamble creates any needed local refs.
    pub fn to_jvalue(&self, val_ident: &proc_macro2::Ident) -> (TokenStream, TokenStream) {
        let tmp = proc_macro2::Ident::new(&format!("__jv_{val_ident}"), proc_macro2::Span::call_site());
        match self {
            // Cast via jboolean so this compiles when jboolean = bool (Android) or u8 (other).
            Self::Bool => (
                quote!(),
                quote!(::jni_high::__private::JValue::Bool(#val_ident as ::jni_high::__private::jboolean)),
            ),
            Self::I8 => (quote!(), quote!(::jni_high::__private::JValue::Byte(#val_ident))),
            Self::I16 => (quote!(), quote!(::jni_high::__private::JValue::Short(#val_ident))),
            Self::I32 => (quote!(), quote!(::jni_high::__private::JValue::Int(#val_ident))),
            Self::I64 => (quote!(), quote!(::jni_high::__private::JValue::Long(#val_ident))),
            Self::F32 => (quote!(), quote!(::jni_high::__private::JValue::Float(#val_ident))),
            Self::F64 => (quote!(), quote!(::jni_high::__private::JValue::Double(#val_ident))),
            Self::Char => (quote!(), quote!(::jni_high::__private::JValue::Char(#val_ident as u16))),
            Self::BorrowedStr => {
                let preamble = quote! {
                    let #tmp = __env.new_string(#val_ident).map_err(::jni_high::BridgeError::from)?;
                };
                (preamble, quote!(::jni_high::__private::JValue::Object(&*#tmp)))
            }
            Self::OwnedString => {
                let preamble = quote! {
                    let #tmp = __env.new_string(&#val_ident).map_err(::jni_high::BridgeError::from)?;
                };
                (preamble, quote!(::jni_high::__private::JValue::Object(&*#tmp)))
            }
            Self::OptionString => {
                let preamble = quote! {
                    let #tmp = match #val_ident.as_deref() {
                        Some(s) => Some(__env.new_string(s).map_err(::jni_high::BridgeError::from)?),
                        None => None,
                    };
                    let #tmp = #tmp.as_ref().map(|s| ::jni_high::__private::JObject::from(s.as_ref()))
                        .unwrap_or(::jni_high::__private::JObject::null());
                };
                (preamble, quote!(::jni_high::__private::JValue::Object(&#tmp)))
            }
            Self::Unit => (quote!(), quote!(::jni_high::__private::JValue::Void)),
        }
    }

    /// Emit code to extract the return value from a `JValueOwned`.
    pub fn extract_return(&self) -> TokenStream {
        match self {
            // z() already returns bool; no need to compare with 0.
            Self::Bool => quote!(__ret.z().map_err(::jni_high::BridgeError::from)),
            Self::I8 => quote!(__ret.b().map_err(::jni_high::BridgeError::from)),
            Self::I16 => quote!(__ret.s().map_err(::jni_high::BridgeError::from)),
            Self::I32 => quote!(__ret.i().map_err(::jni_high::BridgeError::from)),
            Self::I64 => quote!(__ret.j().map_err(::jni_high::BridgeError::from)),
            Self::F32 => quote!(__ret.f().map_err(::jni_high::BridgeError::from)),
            Self::F64 => quote!(__ret.d().map_err(::jni_high::BridgeError::from)),
            Self::Char => quote!(
                __ret
                    .c()
                    .map_err(::jni_high::BridgeError::from)
                    .map(|v| char::from_u32(v as u32).unwrap_or(char::REPLACEMENT_CHARACTER))
            ),
            Self::OwnedString => quote!({
                let __obj = __ret.l().map_err(::jni_high::BridgeError::from)?;
                // Safety: __obj came from a Java method returning String; lifetime tied to __env.
                let __jstr = unsafe { ::jni_high::__private::JString::from_raw(__env, __obj.as_raw()) };
                __jstr.try_to_string(__env).map_err(::jni_high::BridgeError::from)
            }),
            Self::OptionString => quote!({
                let __obj = __ret.l().map_err(::jni_high::BridgeError::from)?;
                if __obj.is_null() {
                    Ok(None)
                } else {
                    // Safety: __obj came from a Java method returning String; lifetime tied to __env.
                    let __jstr = unsafe { ::jni_high::__private::JString::from_raw(__env, __obj.as_raw()) };
                    __jstr
                        .try_to_string(__env)
                        .map_err(::jni_high::BridgeError::from)
                        .map(Some)
                }
            }),
            Self::BorrowedStr => quote!(compile_error!("&str cannot be a return type")),
            Self::Unit => quote!(Ok(())),
        }
    }
}
