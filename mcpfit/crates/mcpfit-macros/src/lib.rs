//! Proc-macro support for `mcpfit`. See `mcpfit/plans/m0.md`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, ItemFn, parse_macro_input};

mod tool;

/// Marks a documented async function as an MCP tool.
///
/// Generates a sibling module (`mod <name>`) exposing the original handler and
/// a `pub const TOOL: ::mcpfit::ToolSpec`. Register with `Server::tool(name::TOOL)`.
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::from(attr),
            "#[tool] does not accept arguments",
        )
        .to_compile_error()
        .into();
    }
    let item = parse_macro_input!(item as ItemFn);
    match tool::expand(item) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Derives [`mcpfit::StructuredObject`] for structs.
///
/// Performs no validation of the runtime JSON shape.
#[proc_macro_derive(StructuredObject)]
pub fn derive_structured_object(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    if !matches!(input.data, Data::Struct(_)) {
        return syn::Error::new_spanned(
            &input.ident,
            "StructuredObject can only be derived for structs",
        )
        .to_compile_error()
        .into();
    }

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics ::mcpfit::StructuredObject for #name #ty_generics #where_clause {}
    }
    .into()
}
