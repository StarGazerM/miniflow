#[path = "../../src/driver.rs"]
mod driver;
#[path = "../../src/syntax.rs"]
mod syntax;

pub(crate) fn compile_canonical(tokens: proc_macro2::TokenStream) -> syn::Result<String> {
    let emitted = driver::compile(tokens)?;
    let file: syn::File = syn::parse2(emitted)?;
    Ok(prettyplease::unparse(&file))
}
