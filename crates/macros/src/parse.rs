use proc_macro2::TokenStream;
use syn::{
    spanned::Spanned, Error, FnArg, GenericArgument, Item, ItemTrait, PathArguments, ReturnType,
    TraitItem, TraitItemFn, Type,
};

pub(crate) struct ServiceInput {
    pub(crate) trait_item: ItemTrait,
}

pub(crate) fn parse_service(attr: TokenStream, item: TokenStream) -> syn::Result<ServiceInput> {
    if !attr.is_empty() {
        return Err(Error::new(
            attr.span(),
            "`#[fittings::service]` does not accept arguments",
        ));
    }

    let parsed_item: Item = syn::parse2(item)?;
    let trait_item = match parsed_item {
        Item::Trait(item_trait) => item_trait,
        other => {
            return Err(Error::new(
                other.span(),
                "`#[fittings::service]` can only be applied to traits",
            ))
        }
    };

    validate_service_trait(&trait_item)?;

    Ok(ServiceInput { trait_item })
}

fn validate_service_trait(trait_item: &ItemTrait) -> syn::Result<()> {
    let mut errors: Option<Error> = None;

    for item in &trait_item.items {
        let result = match item {
            TraitItem::Fn(method) => validate_method_signature(method),
            TraitItem::Type(assoc_type) => Err(Error::new(
                assoc_type.span(),
                "service traits cannot declare associated types",
            )),
            TraitItem::Const(assoc_const) => Err(Error::new(
                assoc_const.span(),
                "service traits cannot declare associated consts",
            )),
            _ => Ok(()),
        };

        if let Err(error) = result {
            if let Some(existing) = &mut errors {
                existing.combine(error);
            } else {
                errors = Some(error);
            }
        }
    }

    if let Some(error) = errors {
        Err(error)
    } else {
        Ok(())
    }
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
