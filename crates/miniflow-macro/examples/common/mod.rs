pub(crate) fn compile_canonical(tokens: proc_macro2::TokenStream) -> syn::Result<String> {
    let emitted = miniflow_core::compile(tokens)?;
    let file: syn::File = syn::parse2(emitted)?;
    Ok(prettyplease::unparse(&file))
}
