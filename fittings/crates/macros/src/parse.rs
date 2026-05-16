use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use syn::{
    spanned::Spanned, Error, FnArg, GenericArgument, Item, ItemTrait, Path, PathArguments,
    ReturnType, TraitItem, TraitItemFn, Type,
};

pub(crate) struct ServiceInput {
    pub(crate) trait_item: ItemTrait,
    pub(crate) method_wire_names: BTreeMap<String, String>,
}

pub(crate) fn parse_service(attr: TokenStream, item: TokenStream) -> syn::Result<ServiceInput> {
    if !attr.is_empty() {
        return Err(Error::new(
            attr.span(),
            "`#[fittings::service]` does not accept arguments",
        ));
    }

    let parsed_item: Item = syn::parse2(item)?;
    let mut trait_item = match parsed_item {
        Item::Trait(item_trait) => item_trait,
        other => {
            return Err(Error::new(
                other.span(),
                "`#[fittings::service]` can only be applied to traits",
            ))
        }
    };

    let method_wire_names = validate_service_trait(&mut trait_item)?;

    Ok(ServiceInput {
        trait_item,
        method_wire_names,
    })
}

fn validate_service_trait(trait_item: &mut ItemTrait) -> syn::Result<BTreeMap<String, String>> {
    let mut errors: Option<Error> = None;
    let mut method_wire_names = BTreeMap::new();
    let mut wire_to_method = BTreeMap::new();

    for item in &mut trait_item.items {
        let result = match item {
            TraitItem::Fn(method) => validate_service_method(method),
            TraitItem::Type(assoc_type) => Err(Error::new(
                assoc_type.span(),
                "service traits cannot declare associated types",
            )),
            TraitItem::Const(assoc_const) => Err(Error::new(
                assoc_const.span(),
                "service traits cannot declare associated consts",
            )),
            _ => Ok(None),
        };

        match result {
            Ok(Some(wire_name)) => {
                let method_name = item_method_name(item)
                    .expect("method name should be present when validate_service_method succeeds");

                if let Some(existing) =
                    wire_to_method.insert(wire_name.clone(), method_name.clone())
                {
                    let duplicate_error = Error::new(
                        item.span(),
                        format!(
                            "duplicate service wire method name `{}`; already used by `{}`",
                            wire_name, existing
                        ),
                    );

                    if let Some(current) = &mut errors {
                        current.combine(duplicate_error);
                    } else {
                        errors = Some(duplicate_error);
                    }
                }

                method_wire_names.insert(method_name, wire_name);
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(existing) = &mut errors {
                    existing.combine(error);
                } else {
                    errors = Some(error);
                }
            }
        }
    }

    if let Some(error) = errors {
        Err(error)
    } else {
        Ok(method_wire_names)
    }
}

fn item_method_name(item: &TraitItem) -> Option<String> {
    match item {
        TraitItem::Fn(method) => Some(method.sig.ident.to_string()),
        _ => None,
    }
}

fn validate_service_method(method: &mut TraitItemFn) -> syn::Result<Option<String>> {
    let wire_name = extract_wire_name_override(method)?;
    validate_method_signature(method)?;
    Ok(Some(
        wire_name.unwrap_or_else(|| method.sig.ident.to_string()),
    ))
}

fn extract_wire_name_override(method: &mut TraitItemFn) -> syn::Result<Option<String>> {
    let mut wire_name: Option<String> = None;
    let mut retained_attrs = Vec::with_capacity(method.attrs.len());

    for attr in method.attrs.drain(..) {
        if !is_fittings_method_attr(attr.path()) {
            retained_attrs.push(attr);
            continue;
        }

        if wire_name.is_some() {
            return Err(Error::new(
                attr.span(),
                "duplicate `#[fittings::method(...)]` attribute",
            ));
        }

        let mut parsed_name: Option<syn::LitStr> = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if parsed_name.is_some() {
                    return Err(meta.error("duplicate `name` argument"));
                }

                parsed_name = Some(meta.value()?.parse()?);
                return Ok(());
            }

            Err(meta.error("unsupported `fittings::method` argument; expected `name = \"...\"`"))
        })?;

        let Some(name_lit) = parsed_name else {
            return Err(Error::new(
                attr.span(),
                "`#[fittings::method(...)]` requires `name = \"...\"`",
            ));
        };

        let parsed_wire_name = name_lit.value();
        if parsed_wire_name.is_empty() {
            return Err(Error::new(
                name_lit.span(),
                "method wire name cannot be empty",
            ));
        }

        wire_name = Some(parsed_wire_name);
    }

    method.attrs = retained_attrs;

    Ok(wire_name)
}

fn is_fittings_method_attr(path: &Path) -> bool {
    path.segments.len() == 2
        && path.segments[0].ident == "fittings"
        && path.segments[1].ident == "method"
}

fn validate_method_signature(method: &TraitItemFn) -> syn::Result<()> {
    if method.sig.asyncness.is_none() {
        return Err(Error::new(
            method.sig.span(),
            "service methods must be declared as `async fn`",
        ));
    }

    if !method.sig.generics.params.is_empty() {
        return Err(Error::new(
            method.sig.generics.span(),
            "service methods cannot declare generic parameters",
        ));
    }

    if method.sig.inputs.len() != 2 {
        return Err(Error::new(
            method.sig.inputs.span(),
            "service methods must have signature `async fn name(&self, params: P) -> Result<R, FittingsError>`",
        ));
    }

    match method.sig.inputs.first() {
        Some(FnArg::Receiver(receiver))
            if receiver.reference.is_some() && receiver.mutability.is_none() => {}
        _ => {
            return Err(Error::new(
                method.sig.inputs.span(),
                "service methods must take `&self` as the first parameter",
            ))
        }
    }

    match method.sig.inputs.iter().nth(1) {
        Some(FnArg::Typed(_)) => {}
        _ => {
            return Err(Error::new(
                method.sig.inputs.span(),
                "service methods must take exactly one `params` argument after `&self`",
            ))
        }
    }

    let result_type = match &method.sig.output {
        ReturnType::Type(_, ty) => ty,
        ReturnType::Default => {
            return Err(Error::new(
                method.sig.span(),
                "service methods must return `Result<R, FittingsError>`",
            ))
        }
    };

    let Type::Path(result_path) = &**result_type else {
        return Err(Error::new(
            result_type.span(),
            "service methods must return `Result<R, FittingsError>`",
        ));
    };

    let Some(result_segment) = result_path.path.segments.last() else {
        return Err(Error::new(
            result_type.span(),
            "service methods must return `Result<R, FittingsError>`",
        ));
    };

    if result_segment.ident != "Result" {
        return Err(Error::new(
            result_type.span(),
            "service methods must return `Result<R, FittingsError>`",
        ));
    }

    let PathArguments::AngleBracketed(result_args) = &result_segment.arguments else {
        return Err(Error::new(
            result_segment.span(),
            "service methods must return `Result<R, FittingsError>`",
        ));
    };

    match result_args.args.len() {
        1 => {
            // Allow `Result<T>` aliases with default error type parameters,
            // e.g. `fittings::Result<T>`.
        }
        2 => {
            let error_arg = result_args
                .args
                .iter()
                .nth(1)
                .expect("Result with two generic arguments has an error argument");

            let GenericArgument::Type(Type::Path(error_type_path)) = error_arg else {
                return Err(Error::new(
                    error_arg.span(),
                    "service method error type must be `FittingsError`",
                ));
            };

            let Some(error_ident) = error_type_path
                .path
                .segments
                .last()
                .map(|segment| &segment.ident)
            else {
                return Err(Error::new(
                    error_arg.span(),
                    "service method error type must be `FittingsError`",
                ));
            };

            if error_ident != "FittingsError" {
                return Err(Error::new(
                    error_arg.span(),
                    "service method error type must be `FittingsError`",
                ));
            }
        }
        _ => {
            return Err(Error::new(
                result_args.span(),
                "service methods must return `Result<R, FittingsError>` or `fittings::Result<R>`",
            ));
        }
    }

    Ok(())
}
