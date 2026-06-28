use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Ident, LitStr, Token, Type};

// ---- DSL AST ----------------------------------------------------------------

pub struct BridgeBlock {
    pub dex_expr: Expr,
    pub classes: Vec<BridgeClass>,
}

pub struct BridgeClass {
    pub rust_name: Ident,
    pub java_name: String, // dot or slash notation from user, stored as-is
    pub methods: Vec<BridgeMethod>,
}

pub enum BridgeMethod {
    Static(StaticMethod),
    Callback(CallbackMethod),
}

pub struct StaticMethod {
    pub no_activity: bool,
    pub java_name_override: Option<String>,
    pub rust_name: Ident,
    pub params: Vec<MethodParam>,
    pub ret_ty: Option<Type>,
}

pub struct CallbackMethod {
    pub rust_name: Ident,
    pub params: Vec<MethodParam>,
    pub ret_ty: Option<Type>,
}

pub struct MethodParam {
    pub name: Ident,
    pub ty: Type,
}

// ---- Parsing ----------------------------------------------------------------

impl Parse for BridgeBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // dex = EXPR ,
        let dex_ident: Ident = input.parse()?;
        if dex_ident != "dex" {
            return Err(syn::Error::new(dex_ident.span(), "expected `dex = <expr>`"));
        }
        input.parse::<Token![=]>()?;
        let dex_expr: Expr = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut classes = Vec::new();
        while !input.is_empty() {
            // Consume optional trailing comma between class blocks
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                continue;
            }
            classes.push(input.parse::<BridgeClass>()?);
        }
        Ok(Self { dex_expr, classes })
    }
}

impl Parse for BridgeClass {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // class NAME { ... }
        let class_kw: Ident = input.parse()?;
        if class_kw != "class" {
            return Err(syn::Error::new(class_kw.span(), "expected `class <Name> { ... }`"));
        }
        let rust_name: Ident = input.parse()?;

        let content;
        syn::braced!(content in input);

        // java_name = "..." ,
        let jn_ident: Ident = content.parse()?;
        if jn_ident != "java_name" {
            return Err(syn::Error::new(jn_ident.span(), "expected `java_name = \"...\"`"));
        }
        content.parse::<Token![=]>()?;
        let java_name_lit: LitStr = content.parse()?;
        let java_name = java_name_lit.value();
        // consume optional comma after java_name
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }

        let mut methods = Vec::new();
        while !content.is_empty() {
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
                continue;
            }
            methods.push(content.parse::<BridgeMethod>()?);
        }
        Ok(Self {
            rust_name,
            java_name,
            methods,
        })
    }
}

impl Parse for BridgeMethod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;

        let mut no_activity = false;
        let mut java_name_override: Option<String> = None;

        for attr in &attrs {
            if attr.path().is_ident("no_activity") {
                no_activity = true;
            } else if attr.path().is_ident("java_name") {
                let lit: LitStr = attr.parse_args()?;
                java_name_override = Some(lit.value());
            }
        }

        // `static fn` or `callback fn`
        // `static` is a Rust keyword so it can't be parsed as Ident; handle it via peek/Token![].
        if input.peek(Token![static]) {
            input.parse::<Token![static]>()?;
            input.parse::<Token![fn]>()?;
            let rust_name: Ident = input.parse()?;
            let params = parse_params(input)?;
            let ret_ty = parse_ret_ty(input)?;
            input.parse::<Token![;]>()?;
            return Ok(Self::Static(StaticMethod {
                no_activity,
                java_name_override,
                rust_name,
                params,
                ret_ty,
            }));
        }

        let kw: Ident = input.parse()?;
        if kw == "callback" {
            input.parse::<Token![fn]>()?;
            let rust_name: Ident = input.parse()?;
            let params = parse_params(input)?;
            let ret_ty = parse_ret_ty(input)?;
            input.parse::<Token![;]>()?;
            Ok(Self::Callback(CallbackMethod {
                rust_name,
                params,
                ret_ty,
            }))
        } else {
            Err(syn::Error::new(
                kw.span(),
                format!("expected `static` or `callback`, got `{kw}`"),
            ))
        }
    }
}

fn parse_params(input: ParseStream) -> syn::Result<Vec<MethodParam>> {
    let content;
    syn::parenthesized!(content in input);
    let punct: Punctuated<MethodParam, Token![,]> = content.parse_terminated(MethodParam::parse, Token![,])?;
    Ok(punct.into_iter().collect())
}

impl Parse for MethodParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(Self { name, ty })
    }
}

fn parse_ret_ty(input: ParseStream) -> syn::Result<Option<Type>> {
    if input.peek(Token![->]) {
        input.parse::<Token![->]>()?;
        Ok(Some(input.parse::<Type>()?))
    } else {
        Ok(None)
    }
}
