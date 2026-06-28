use heck::ToLowerCamelCase;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::parse::{BridgeBlock, BridgeClass, BridgeMethod, CallbackMethod, MethodParam, StaticMethod};
use crate::types::BridgeType;

pub fn generate(block: &BridgeBlock) -> syn::Result<TokenStream> {
    let dex_expr = &block.dex_expr;
    let mut out = TokenStream::new();
    for class in &block.classes {
        out.extend(generate_class(class, dex_expr)?);
    }
    Ok(out)
}

fn generate_class(class: &BridgeClass, dex_expr: &syn::Expr) -> syn::Result<TokenStream> {
    let rust_name = &class.rust_name;
    // java_name stays in dot notation for ClassLoader.findClass(); slash form unused in generated code.
    let java_name = &class.java_name;
    let mod_name = format_ident!("__jni_high_{}", rust_name.to_string().to_lowercase());

    let mut statics_src: Vec<&StaticMethod> = Vec::new();
    let mut callbacks_src: Vec<&CallbackMethod> = Vec::new();
    for m in &class.methods {
        match m {
            BridgeMethod::Static(s) => statics_src.push(s),
            BridgeMethod::Callback(c) => callbacks_src.push(c),
        }
    }

    let callback_statics = generate_callback_statics(&callbacks_src)?;
    let extern_fns = generate_extern_fns(rust_name, &mod_name, &callbacks_src)?;
    let register_fn = generate_register_fn(rust_name, &callbacks_src, java_name)?;
    let class_init_fn = generate_class_init_fn(&mod_name, dex_expr, java_name);
    let method_impls = generate_static_methods(&statics_src)?;
    let set_handlers = generate_set_handlers(&mod_name, &callbacks_src)?;

    Ok(quote! {
        #[doc(hidden)]
        mod #mod_name {
            use super::*;

            pub(super) static CLASS: ::std::sync::OnceLock<
                ::jni_high::__private::Global<::jni_high::__private::JClass<'static>>
            > = ::std::sync::OnceLock::new();

            #callback_statics
        }

        #extern_fns

        pub struct #rust_name;

        impl #rust_name {
            #class_init_fn
            #register_fn
            #method_impls
            #set_handlers
        }
    })
}

// ---- Callback handler statics -----------------------------------------------

fn generate_callback_statics(callbacks: &[&CallbackMethod]) -> syn::Result<TokenStream> {
    let mut out = TokenStream::new();
    for cb in callbacks {
        let handler_ident = format_ident!("HANDLER_{}", cb.rust_name.to_string().to_uppercase());
        let param_types = callback_handler_types(&cb.params)?;
        let fn_ty = fn_type_tokens(&param_types);
        out.extend(quote! {
            pub(super) static #handler_ident: ::std::sync::Mutex<Option<Box<#fn_ty>>> =
                ::std::sync::Mutex::new(None);
        });
    }
    Ok(out)
}

// ---- Extern callback functions ----------------------------------------------

fn generate_extern_fns(rust_name: &Ident, mod_name: &Ident, callbacks: &[&CallbackMethod]) -> syn::Result<TokenStream> {
    callbacks.iter().try_fold(TokenStream::new(), |mut acc, cb| {
        acc.extend(generate_extern_fn(rust_name, mod_name, cb)?);
        Ok(acc)
    })
}

fn generate_extern_fn(rust_name: &Ident, mod_name: &Ident, cb: &CallbackMethod) -> syn::Result<TokenStream> {
    let fn_name = format_ident!("__{}_{}_native", rust_name.to_string().to_lowercase(), cb.rust_name);
    let handler_ident = format_ident!("HANDLER_{}", cb.rust_name.to_string().to_uppercase());
    let extern_params = build_extern_params(&cb.params)?;
    let body = build_callback_body(mod_name, &handler_ident, &fn_name, &cb.params)?;

    Ok(quote! {
        #[allow(non_snake_case)]
        unsafe extern "system" fn #fn_name<'__local>(
            mut __unowned: ::jni_high::__private::EnvUnowned<'__local>,
            _class: ::jni_high::__private::JClass<'__local>,
            #(#extern_params),*
        ) {
            #body
        }
    })
}

fn build_extern_params(params: &[MethodParam]) -> syn::Result<Vec<TokenStream>> {
    params
        .iter()
        .map(|param| {
            let bt = BridgeType::from_syn(&param.ty)?;
            let pname = &param.name;
            let pty = bt.callback_param_type();
            Ok(quote!(#pname: #pty))
        })
        .collect()
}

fn build_callback_body(
    mod_name: &Ident,
    handler_ident: &Ident,
    fn_name: &Ident,
    params: &[MethodParam],
) -> syn::Result<TokenStream> {
    let handler_call = build_handler_call(params);
    if params.is_empty() {
        return Ok(quote! {
            let _ = __unowned;
            if let Ok(guard) = #mod_name::#handler_ident.lock() {
                if let Some(handler) = guard.as_ref() {
                    #handler_call;
                }
            }
        });
    }
    let (conv_idents, conv_exprs) = build_conv_exprs(params)?;
    let (tuple_pat, tuple_expr) = build_conv_tuple(&conv_idents, &conv_exprs);
    Ok(quote! {
        let __outcome = __unowned.with_env_no_catch(|__env| {
            #tuple_expr
        }).into_outcome();

        let #tuple_pat = match __outcome {
            ::jni_high::__private::Outcome::Ok(vals) => vals,
            _ => {
                ::log::error!("jni-high: callback {} param conversion failed", stringify!(#fn_name));
                return;
            }
        };

        if let Ok(guard) = #mod_name::#handler_ident.lock() {
            if let Some(handler) = guard.as_ref() {
                #handler_call;
            }
        }
    })
}

fn build_handler_call(params: &[MethodParam]) -> TokenStream {
    if params.is_empty() {
        return quote!(handler());
    }
    let conv_idents: Vec<Ident> = (0..params.len()).map(|i| format_ident!("__c{}", i)).collect();
    quote!(handler(#(#conv_idents),*))
}

fn build_conv_exprs(params: &[MethodParam]) -> syn::Result<(Vec<Ident>, Vec<TokenStream>)> {
    let mut idents = Vec::with_capacity(params.len());
    let mut exprs = Vec::with_capacity(params.len());
    for (idx, param) in params.iter().enumerate() {
        let bt = BridgeType::from_syn(&param.ty)?;
        idents.push(format_ident!("__c{}", idx));
        exprs.push(bt.callback_closure_expr(&param.name));
    }
    Ok((idents, exprs))
}

/// Returns (tuple_pattern, Ok::<_, Error>((expr0, expr1, ...,))) for use inside with_env_no_catch.
fn build_conv_tuple(idents: &[Ident], exprs: &[TokenStream]) -> (TokenStream, TokenStream) {
    debug_assert_eq!(idents.len(), exprs.len());
    debug_assert!(!idents.is_empty());
    if let ([id], [ex]) = (idents, exprs) {
        (
            quote!((#id,)),
            quote!(Ok::<_, ::jni_high::__private::JniSysError>((#ex,))),
        )
    } else {
        let pat = quote!((#(#idents),*));
        let tup = quote!(Ok::<_, ::jni_high::__private::JniSysError>((#(#exprs),*)));
        (pat, tup)
    }
}

// ---- __register_natives -----------------------------------------------------

fn generate_register_fn(
    rust_name: &Ident,
    callbacks: &[&CallbackMethod],
    java_class_name: &str,
) -> syn::Result<TokenStream> {
    if callbacks.is_empty() {
        return Ok(quote! {
            fn __register_natives(
                _loader: &::jni_high::__private::JObject<'_>,
                _env: &mut ::jni::Env<'_>,
            ) -> ::jni_high::BridgeResult<()> {
                Ok(())
            }
        });
    }

    let struct_prefix = rust_name.to_string().to_lowercase();
    let native_entries: Vec<TokenStream> = callbacks
        .iter()
        .map(|cb| build_native_entry(&struct_prefix, cb))
        .collect::<syn::Result<_>>()?;
    let class_name = java_class_name.to_string();

    Ok(quote! {
        fn __register_natives(
            loader: &::jni_high::__private::JObject<'_>,
            env: &mut ::jni_high::__private::Env<'_>,
        ) -> ::jni_high::BridgeResult<()> {
            let __class = ::jni_high::__private::find_class_in_loader(env, loader, #class_name)?;
            let __methods: &[::jni_high::__private::NativeMethod<'_>] = &[
                #(#native_entries),*
            ];
            unsafe { env.register_native_methods(&__class, __methods) }
                .map_err(::jni_high::BridgeError::from)
        }
    })
}

fn build_native_entry(struct_prefix: &str, cb: &CallbackMethod) -> syn::Result<TokenStream> {
    let fn_name = format_ident!("__{}_{}_native", struct_prefix, cb.rust_name);
    let java_method_name = cb.rust_name.to_string().to_lower_camel_case();
    let name_cstr = make_cstr_lit(&java_method_name);

    let mut sig = String::from("(");
    for param in &cb.params {
        sig.push_str(BridgeType::from_syn(&param.ty)?.jni_sig_char());
    }
    let ret_sig = match &cb.ret_ty {
        Some(ty) => BridgeType::from_syn(ty)?.jni_sig_char().to_string(),
        None => "V".to_string(),
    };
    sig.push(')');
    sig.push_str(&ret_sig);
    let sig_cstr = make_cstr_lit(&sig);

    Ok(quote! {
        unsafe {
            ::jni_high::__private::NativeMethod::from_raw_parts(
                unsafe { ::jni_high::__private::JNIStr::from_cstr_unchecked(#name_cstr) },
                unsafe { ::jni_high::__private::JNIStr::from_cstr_unchecked(#sig_cstr) },
                #fn_name as *mut ::std::ffi::c_void,
            )
        }
    })
}

// ---- __class() OnceLock init ------------------------------------------------

fn generate_class_init_fn(mod_name: &Ident, dex_expr: &syn::Expr, java_class_name: &str) -> TokenStream {
    let class_name = java_class_name.to_string();

    quote! {
        fn __class(ctx: &::jni_high::AndroidContext)
            -> ::jni_high::BridgeResult<&'static ::jni_high::__private::Global<::jni_high::__private::JClass<'static>>>
        {
            // OnceLock::get_or_try_init is unstable; use get + set instead.
            // Two racing threads both initializing is harmless: set() fails for the loser,
            // and the winner's value is returned by get() on the next line.
            if let Some(c) = #mod_name::CLASS.get() {
                return Ok(c);
            }
            let __class = ctx.attach(move |__env, __activity| {
                let __loader = ::jni_high::__private::load_dex(__env, __activity, #dex_expr)?;
                Self::__register_natives(&__loader, __env)?;
                let __class_local = ::jni_high::__private::find_class_in_loader(__env, &__loader, #class_name)?;
                Ok(__env.new_global_ref(__class_local).map_err(::jni_high::BridgeError::from)?)
            })?;
            let _ = #mod_name::CLASS.set(__class);
            Ok(#mod_name::CLASS.get().expect("CLASS was just set or won the race"))
        }
    }
}

// ---- Static method calls ----------------------------------------------------

fn generate_static_methods(methods: &[&StaticMethod]) -> syn::Result<TokenStream> {
    methods.iter().try_fold(TokenStream::new(), |mut acc, m| {
        acc.extend(generate_static_method(m)?);
        Ok(acc)
    })
}

fn generate_static_method(m: &StaticMethod) -> syn::Result<TokenStream> {
    let rust_fn_name = &m.rust_name;
    let java_name = m
        .java_name_override
        .clone()
        .unwrap_or_else(|| m.rust_name.to_string().to_lower_camel_case());
    let java_name_cstr = make_cstr_lit(&java_name);

    let ret_bt = parse_return_type(m)?;
    let (mut sig, java_type_tokens) = build_jni_param_sig(m)?;
    sig.push_str(ret_bt.jni_sig_char());
    let sig_cstr = make_cstr_lit(&sig);

    let param_decls = build_param_decls(m)?;
    let (jvalue_preambles, jvalue_args) = build_jvalue_args(m)?;
    let ret_java_type = ret_bt.return_type_token();
    let extract_return = ret_bt.extract_return();
    let rust_ret_type = ret_bt.rust_type_token();

    let inner_closure = build_static_call_closure(
        m,
        &java_name_cstr,
        &sig_cstr,
        &java_type_tokens,
        &jvalue_preambles,
        &jvalue_args,
        &ret_java_type,
        &extract_return,
    );

    Ok(quote! {
        pub fn #rust_fn_name(#(#param_decls),*) -> ::jni_high::BridgeResult<#rust_ret_type> {
            let ctx = ::jni_high::AndroidContext::get()
                .ok_or(::jni_high::BridgeError::ContextNotInitialized)?;
            let ctx: &'static ::jni_high::AndroidContext = ctx;
            #inner_closure
        }
    })
}

fn parse_return_type(m: &StaticMethod) -> syn::Result<BridgeType> {
    match &m.ret_ty {
        Some(ty) => BridgeType::from_syn(ty),
        None => Ok(BridgeType::Unit),
    }
}

fn build_jni_param_sig(m: &StaticMethod) -> syn::Result<(String, Vec<TokenStream>)> {
    let mut sig = String::from("(");
    let mut java_types = Vec::new();
    if !m.no_activity {
        sig.push_str("Landroid/app/Activity;");
        java_types.push(quote!(::jni_high::__private::JavaType::Object));
    }
    for param in &m.params {
        let bt = BridgeType::from_syn(&param.ty)?;
        sig.push_str(bt.jni_sig_char());
        java_types.push(bt.java_type_token());
    }
    sig.push(')');
    Ok((sig, java_types))
}

fn build_param_decls(m: &StaticMethod) -> syn::Result<Vec<TokenStream>> {
    m.params
        .iter()
        .map(|param| {
            let bt = BridgeType::from_syn(&param.ty)?;
            let pname = &param.name;
            let pty = bt.rust_type_token();
            Ok(quote!(#pname: #pty))
        })
        .collect()
}

fn build_jvalue_args(m: &StaticMethod) -> syn::Result<(Vec<TokenStream>, Vec<TokenStream>)> {
    let mut preambles = Vec::new();
    let mut args = Vec::new();
    if !m.no_activity {
        args.push(quote!(::jni_high::__private::JValue::Object(__activity)));
    }
    for param in &m.params {
        let bt = BridgeType::from_syn(&param.ty)?;
        let (pre, jv) = bt.to_jvalue(&param.name);
        preambles.push(pre);
        args.push(jv);
    }
    Ok((preambles, args))
}

#[expect(clippy::too_many_arguments)]
fn build_static_call_closure(
    m: &StaticMethod,
    java_name_cstr: &TokenStream,
    sig_cstr: &TokenStream,
    java_type_tokens: &[TokenStream],
    jvalue_preambles: &[TokenStream],
    jvalue_args: &[TokenStream],
    ret_java_type: &TokenStream,
    extract_return: &TokenStream,
) -> TokenStream {
    if m.no_activity {
        quote! {
            ctx.attach(move |__env, _| {
                let __class = Self::__class(ctx)?;
                #(#jvalue_preambles)*
                let __ret = __env.call_static_method(
                    __class,
                    unsafe { ::jni_high::__private::JNIStr::from_cstr_unchecked(#java_name_cstr) },
                    unsafe {
                        ::jni_high::__private::MethodSignature::from_raw_parts(
                            ::jni_high::__private::JNIStr::from_cstr_unchecked(#sig_cstr),
                            &[#(#java_type_tokens),*],
                            #ret_java_type,
                        )
                    },
                    &[#(#jvalue_args),*],
                ).map_err(::jni_high::BridgeError::from)?;
                #extract_return
            })
        }
    } else {
        quote! {
            ctx.attach(move |__env, __activity| {
                let __class = Self::__class(ctx)?;
                #(#jvalue_preambles)*
                let __ret = __env.call_static_method(
                    __class,
                    unsafe { ::jni_high::__private::JNIStr::from_cstr_unchecked(#java_name_cstr) },
                    unsafe {
                        ::jni_high::__private::MethodSignature::from_raw_parts(
                            ::jni_high::__private::JNIStr::from_cstr_unchecked(#sig_cstr),
                            &[#(#java_type_tokens),*],
                            #ret_java_type,
                        )
                    },
                    &[#(#jvalue_args),*],
                ).map_err(::jni_high::BridgeError::from)?;
                #extract_return
            })
        }
    }
}

// ---- set_* handler methods --------------------------------------------------

fn generate_set_handlers(mod_name: &Ident, callbacks: &[&CallbackMethod]) -> syn::Result<TokenStream> {
    let mut out = TokenStream::new();
    for cb in callbacks {
        let set_fn = format_ident!("set_{}", cb.rust_name);
        let handler_ident = format_ident!("HANDLER_{}", cb.rust_name.to_string().to_uppercase());
        let param_types = callback_handler_types(&cb.params)?;
        let fn_bound = fn_bound_tokens(&param_types);

        out.extend(quote! {
            pub fn #set_fn<F>(handler: F)
            where
                #fn_bound,
            {
                if let Ok(mut guard) = #mod_name::#handler_ident.lock() {
                    *guard = Some(Box::new(handler));
                }
            }
        });
    }
    Ok(out)
}

// ---- Shared helpers ---------------------------------------------------------

fn callback_handler_types(params: &[MethodParam]) -> syn::Result<Vec<TokenStream>> {
    params
        .iter()
        .map(|p| {
            let bt = BridgeType::from_syn(&p.ty)?;
            Ok(bt.callback_handler_type())
        })
        .collect()
}

fn fn_type_tokens(param_types: &[TokenStream]) -> TokenStream {
    if param_types.is_empty() {
        quote!(dyn Fn() + Send + Sync + 'static)
    } else {
        quote!(dyn Fn(#(#param_types),*) + Send + Sync + 'static)
    }
}

fn fn_bound_tokens(param_types: &[TokenStream]) -> TokenStream {
    if param_types.is_empty() {
        quote!(F: Fn() + Send + Sync + 'static)
    } else {
        quote!(F: Fn(#(#param_types),*) + Send + Sync + 'static)
    }
}

fn make_cstr_lit(s: &str) -> TokenStream {
    let cstring = std::ffi::CString::new(s.as_bytes())
        .unwrap_or_else(|_| panic!("jni-high codegen: embedded null in string `{s}`"));
    let lit = proc_macro2::Literal::c_string(&cstring);
    quote!(#lit)
}
