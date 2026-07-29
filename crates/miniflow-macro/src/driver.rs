//! Composition of the surface parser with the syntax-neutral compiler kernel.

use miniflow_core::compiler::{Layer, Registry};
use miniflow_core::{Compiler, ReadSource};
use proc_macro2::TokenStream;
use syn::Result;

use super::syntax;

pub(crate) struct SurfaceSyntax;

impl Layer for SurfaceSyntax {
    fn install(&self, registry: &mut Registry) -> Result<()> {
        registry.define::<ReadSource, _>(|_, tokens| syntax::parse(tokens))
    }
}

fn compiler() -> Result<Compiler> {
    let mut compiler = Compiler::base()?;
    compiler.install(&SurfaceSyntax)?;
    Ok(compiler)
}

pub(crate) fn compile(tokens: TokenStream) -> Result<TokenStream> {
    compiler()?.compile(tokens)
}
