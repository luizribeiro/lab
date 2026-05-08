use syn::{Attribute, Expr, ExprLit, FnArg, ItemFn, Lit, Meta, PatType, Type, spanned::Spanned};

#[allow(dead_code)]
pub(crate) struct ParsedTool {
    pub(crate) name: syn::Ident,
    pub(crate) description: String,
    pub(crate) args_ty: Box<Type>,
    pub(crate) cx_arg: Option<Box<Type>>,
    pub(crate) return_ty: Box<Type>,
}

#[allow(dead_code)]
pub(crate) fn parse(item: &ItemFn) -> syn::Result<ParsedTool> {
    if item.sig.asyncness.is_none() {
        return Err(syn::Error::new(
            item.sig.fn_token.span(),
            "#[tool] functions must be async",
        ));
    }

    let description = extract_doc(&item.attrs)?;
    let (args_ty, cx_arg) = extract_args(item)?;
    let return_ty = extract_return(item)?;

    Ok(ParsedTool {
        name: item.sig.ident.clone(),
        description,
        args_ty,
        cx_arg,
        return_ty,
    })
}

fn extract_doc(attrs: &[Attribute]) -> syn::Result<String> {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let Meta::NameValue(nv) = &attr.meta
            && let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = &nv.value
        {
            lines.push(s.value().trim().to_owned());
        }
    }
    let description = lines.join("\n").trim().to_owned();
    if description.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[tool] functions require a non-empty doc comment",
        ));
    }
    Ok(description)
}

fn extract_args(item: &ItemFn) -> syn::Result<(Box<Type>, Option<Box<Type>>)> {
    let mut typed = Vec::new();
    for arg in &item.sig.inputs {
        match arg {
            FnArg::Receiver(r) => {
                return Err(syn::Error::new(
                    r.span(),
                    "#[tool] functions cannot take `self`",
                ));
            }
            FnArg::Typed(PatType { ty, .. }) => typed.push(ty.clone()),
        }
    }
    match typed.len() {
        1 => Ok((typed.remove(0), None)),
        2 => {
            let args = typed.remove(0);
            let cx = typed.remove(0);
            Ok((args, Some(cx)))
        }
        _ => Err(syn::Error::new(
            item.sig.paren_token.span.span(),
            "#[tool] functions must take `args: A` or `args: A, cx: Cx`",
        )),
    }
}

fn extract_return(item: &ItemFn) -> syn::Result<Box<Type>> {
    match &item.sig.output {
        syn::ReturnType::Type(_, ty) => Ok(ty.clone()),
        syn::ReturnType::Default => Err(syn::Error::new(
            item.sig.span(),
            "#[tool] functions must return `Result<T>`",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;
    use syn::parse_quote;

    fn ty_string(ty: &Type) -> String {
        ty.to_token_stream().to_string()
    }

    #[test]
    fn parses_one_arg_tool() {
        let item: ItemFn = parse_quote! {
            /// Adds two numbers.
            async fn add(args: AddArgs) -> Result<f64> { Ok(args.a + args.b) }
        };
        let parsed = parse(&item).unwrap();
        assert_eq!(parsed.name, "add");
        assert_eq!(parsed.description, "Adds two numbers.");
        assert_eq!(ty_string(&parsed.args_ty), "AddArgs");
        assert!(parsed.cx_arg.is_none());
        assert_eq!(ty_string(&parsed.return_ty), "Result < f64 >");
    }

    #[test]
    fn parses_two_arg_tool_with_cx() {
        let item: ItemFn = parse_quote! {
            /// Sleeps then returns.
            /// Cancellable.
            async fn nap(args: NapArgs, cx: Cx) -> Result<String> { Ok("ok".into()) }
        };
        let parsed = parse(&item).unwrap();
        assert_eq!(parsed.description, "Sleeps then returns.\nCancellable.");
        assert_eq!(ty_string(parsed.cx_arg.as_deref().unwrap()), "Cx");
    }

    fn err_message(item: &ItemFn) -> String {
        match parse(item) {
            Ok(_) => panic!("expected error"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn rejects_non_async() {
        let item: ItemFn = parse_quote! {
            /// Doc.
            fn sync_fn(args: ()) -> Result<()> { Ok(()) }
        };
        assert!(err_message(&item).contains("async"));
    }

    #[test]
    fn rejects_missing_doc() {
        let item: ItemFn = parse_quote! {
            async fn no_doc(args: ()) -> Result<()> { Ok(()) }
        };
        assert!(err_message(&item).contains("doc comment"));
    }

    #[test]
    fn rejects_default_return() {
        let item: ItemFn = parse_quote! {
            /// Doc.
            async fn no_ret(args: ()) {}
        };
        assert!(err_message(&item).contains("Result<T>"));
    }

    #[test]
    fn rejects_unsupported_arity() {
        let item: ItemFn = parse_quote! {
            /// Doc.
            async fn three(a: A, b: B, c: C) -> Result<()> { Ok(()) }
        };
        assert!(parse(&item).is_err());
    }
}
