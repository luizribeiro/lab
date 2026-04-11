use proc_macro::TokenStream;

mod expand;
mod parse;

#[proc_macro_attribute]
pub fn service(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_tokens = proc_macro2::TokenStream::from(attr);
    let item_tokens = proc_macro2::TokenStream::from(item);

    match parse::parse_service(attr_tokens, item_tokens) {
        Ok(parsed) => expand::expand_service(parsed).into(),
        Err(error) => error.to_compile_error().into(),
    }
}
