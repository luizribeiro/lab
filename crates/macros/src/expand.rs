use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{
    parse_quote, Attribute, Expr, ExprLit, FnArg, ItemTrait, Lit, Meta, PatType, ReturnType,
    TraitItem, TraitItemFn, Type, TypePath,
};

use crate::parse::ServiceInput;

pub(crate) fn expand_service(input: ServiceInput) -> proc_macro2::TokenStream {
    let mut trait_item = input.trait_item;
    let trait_ident = trait_item.ident.clone();
    let trait_vis = trait_item.vis.clone();

    let methods = trait_item
        .items
        .iter()
        .filter_map(|item| match item {
            TraitItem::Fn(method) => Some(MethodInfo {
                ident: method.sig.ident.clone(),
                params_type: method_params_type(method),
                result_ok_type: method_result_ok_type(method),
                description: method_description(method),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    rewrite_trait_methods_as_send_futures(&mut trait_item);

    let router_ident = format_ident!("{}Router", trait_ident);
    let trait_snake = to_snake_case(&trait_ident.to_string());
    let service_name = to_kebab_case(&trait_ident.to_string());
    let schema_fn_ident = format_ident!("{}_schema", trait_snake, span = Span::call_site());
    let constructor_ident = format_ident!("into_{}_router", trait_snake, span = Span::call_site());

    let method_schema_items = methods.iter().map(|method| {
        let method_name = method.ident.to_string();
        let params_type = &method.params_type;
        let result_type = &method.result_ok_type;

        let description_expr = if let Some(description) = &method.description {
            quote! { Some(::std::string::String::from(#description)) }
        } else {
            quote! { None }
        };

        quote! {
            ::fittings::MethodSchema {
                name: ::std::string::String::from(#method_name),
                description: #description_expr,
                params_schema: Some(
                    ::fittings::serde_json::to_value(::fittings::schemars::schema_for!(#params_type))
                        .expect("generated service schema: params schema should serialize"),
                ),
                result_schema: Some(
                    ::fittings::serde_json::to_value(::fittings::schemars::schema_for!(#result_type))
                        .expect("generated service schema: result schema should serialize"),
                ),
            }
        }
    });

    let dispatch_arms = methods.iter().map(|method| {
        let method_ident = &method.ident;
        let method_name = method_ident.to_string();
        let params_type = &method.params_type;
        let result_type = &method.result_ok_type;

        quote! {
            #method_name => {
                let decoded_params: #params_type = ::fittings::serde_json::from_value(params)
                    .map_err(|error| ::fittings::FittingsError::invalid_params(format!(
                        "failed to decode params for method `{}`: {}",
                        #method_name,
                        error
                    )))?;

                let result: #result_type = <I as #trait_ident>::#method_ident(&self.inner, decoded_params).await?;

                ::fittings::serde_json::to_value(result).map_err(|error| {
                    ::fittings::FittingsError::internal(format!(
                        "failed to encode result for method `{}`: {}",
                        #method_name,
                        error
                    ))
                })
            }
        }
    });

    quote! {
        #trait_item

        #trait_vis fn #schema_fn_ident() -> ::fittings::ServiceSchema {
            ::fittings::ServiceSchema {
                name: ::std::string::String::from(#service_name),
                methods: vec![#(#method_schema_items,)*],
                config_schema: Some(::fittings::serde_json::json!({
                    "type": "object",
                    "properties": {
                        "log_level": {
                            "type": "string",
                            "enum": ["trace", "debug", "info", "warn", "error"]
                        }
                    },
                    "additionalProperties": false
                })),
            }
        }

        #trait_vis struct #router_ident<I> {
            inner: I,
        }

        #[::fittings::async_trait::async_trait]
        impl<I> ::fittings::MethodRouter for #router_ident<I>
        where
            I: #trait_ident + Send + Sync,
        {
            async fn route(
                &self,
                method: &str,
                params: ::fittings::serde_json::Value,
                _metadata: ::fittings::Metadata,
            ) -> Result<::fittings::serde_json::Value, ::fittings::FittingsError> {
                match method {
                    #(#dispatch_arms,)*
                    _ => Err(::fittings::FittingsError::method_not_found(method.to_string())),
                }
            }
        }

        #trait_vis fn #constructor_ident<I>(inner: I) -> #router_ident<I>
        where
            I: #trait_ident + Send + Sync,
        {
            #router_ident { inner }
        }
    }
}

struct MethodInfo {
    ident: syn::Ident,
    params_type: Type,
    result_ok_type: Type,
    description: Option<String>,
}

fn rewrite_trait_methods_as_send_futures(trait_item: &mut ItemTrait) {
    for item in &mut trait_item.items {
        let TraitItem::Fn(method) = item else {
            continue;
        };

        method.sig.asyncness = None;

        let output_type = match &method.sig.output {
            ReturnType::Type(_, ty) => (**ty).clone(),
            ReturnType::Default => {
                unreachable!(
                    "service method signature validated to return Result<R, FittingsError>"
                )
            }
        };

        method.sig.output = parse_quote! {
            -> impl ::core::future::Future<Output = #output_type> + Send
        };
    }
}

fn method_params_type(method: &TraitItemFn) -> Type {
    let second_arg = method
        .sig
        .inputs
        .iter()
        .nth(1)
        .expect("service method signature validated to have exactly two arguments");

    match second_arg {
        FnArg::Typed(PatType { ty, .. }) => (**ty).clone(),
        FnArg::Receiver(_) => {
            unreachable!("service method signature validated to use a typed params argument")
        }
    }
}

fn method_result_ok_type(method: &TraitItemFn) -> Type {
    let output = match &method.sig.output {
        ReturnType::Type(_, output) => output,
        ReturnType::Default => {
            unreachable!("service method signature validated to return Result<R, FittingsError>")
        }
    };

    let Type::Path(TypePath { path, .. }) = &**output else {
        unreachable!("service method signature validated to return a path type")
    };

    let result_segment = path
        .segments
        .last()
        .expect("service method signature validated to have a Result segment");

    let syn::PathArguments::AngleBracketed(args) = &result_segment.arguments else {
        unreachable!("service method signature validated to use Result<R, E>");
    };

    let ok_arg = args
        .args
        .first()
        .expect("service method signature validated to include an ok type");

    match ok_arg {
        syn::GenericArgument::Type(ok_type) => ok_type.clone(),
        _ => unreachable!("service method signature validated to use a type for Result ok value"),
    }
}

fn method_description(method: &TraitItemFn) -> Option<String> {
    extract_doc_comment(&method.attrs)
}

fn extract_doc_comment(attrs: &[Attribute]) -> Option<String> {
    let mut lines = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }

        let Meta::NameValue(name_value) = &attr.meta else {
            continue;
        };

        let Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) = &name_value.value
        else {
            continue;
        };

        lines.push(value.value().trim().to_string());
    }

    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }

    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn to_snake_case(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut prev_is_lower_or_digit = false;

    for ch in input.chars() {
        if ch.is_uppercase() {
            if prev_is_lower_or_digit {
                output.push('_');
            }

            for lower in ch.to_lowercase() {
                output.push(lower);
            }

            prev_is_lower_or_digit = false;
        } else {
            output.push(ch);
            prev_is_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }

    output
}

fn to_kebab_case(input: &str) -> String {
    to_snake_case(input).replace('_', "-")
}
