use proc_macro2::TokenStream;
use syn::{spanned::Spanned, Error, Item, ItemTrait};

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

    Ok(ServiceInput { trait_item })
}
