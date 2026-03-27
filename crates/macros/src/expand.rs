use quote::quote;

use crate::parse::ServiceInput;

pub(crate) fn expand_service(input: ServiceInput) -> proc_macro2::TokenStream {
    let trait_item = input.trait_item;

    // Commit M1 scaffold: preserve the trait and reserve expansion hooks for later commits.
    quote! {
        #trait_item
    }
}
