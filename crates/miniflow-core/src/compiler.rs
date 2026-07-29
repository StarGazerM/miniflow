//! Open compiler operations and their affine handler chains.
//!
//! A language layer defines an [`Operation`] marker and installs an around
//! handler. The registry erases each chain at rest but restores its input and
//! output types whenever the operation is performed. [`Next::call`] consumes
//! the continuation, so ordinary compiler operations cannot accidentally
//! branch the remainder of compilation.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use proc_macro2::Span;
use syn::Result;

use crate::plan::FactDb;

/// Mutable state shared by compiler operations.
#[derive(Default)]
pub struct CompilerContext {
    facts: FactDb,
}

impl CompilerContext {
    /// Return all compiler facts.
    #[must_use]
    pub const fn facts(&self) -> &FactDb {
        &self.facts
    }

    /// Return all compiler facts for mutation.
    pub const fn facts_mut(&mut self) -> &mut FactDb {
        &mut self.facts
    }
}

/// A named, typed effect that may be handled by compiler layers.
pub trait Operation: 'static {
    /// Value supplied when the operation is performed.
    type Input: 'static;

    /// Value produced by the operation.
    type Output: 'static;

    /// Stable diagnostic name for the operation.
    const NAME: &'static str;
}

type Terminal<O> =
    dyn Fn(&mut CompilerContext, <O as Operation>::Input) -> Result<<O as Operation>::Output>;

type Around<O> = dyn for<'a> Fn(
    &mut CompilerContext,
    <O as Operation>::Input,
    Next<'a, O>,
) -> Result<<O as Operation>::Output>;

struct HandlerChain<O: Operation> {
    terminal: Option<Box<Terminal<O>>>,
    around: Vec<Box<Around<O>>>,
}

impl<O: Operation> Default for HandlerChain<O> {
    fn default() -> Self {
        Self {
            terminal: None,
            around: Vec::new(),
        }
    }
}

/// The remaining implementation of one compiler operation.
///
/// Calling the continuation consumes it. A handler may delegate once or
/// replace the operation by returning without calling it.
pub struct Next<'a, O: Operation> {
    chain: &'a HandlerChain<O>,
    index: usize,
}

impl<O: Operation> Next<'_, O> {
    /// Invoke the next handler, or the operation's terminal implementation.
    ///
    /// # Errors
    ///
    /// Returns any diagnostic produced by the next implementation, or a
    /// registry diagnostic when the operation has no terminal implementation.
    pub fn call(self, context: &mut CompilerContext, input: O::Input) -> Result<O::Output> {
        if let Some(handler) = self.chain.around.get(self.index) {
            return handler(
                context,
                input,
                Next {
                    chain: self.chain,
                    index: self.index + 1,
                },
            );
        }
        let terminal = self.chain.terminal.as_ref().ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                format!(
                    "compiler operation `{}` has no terminal implementation",
                    O::NAME
                ),
            )
        })?;
        terminal(context, input)
    }
}

/// Type-erased collection of open compiler-operation handlers.
#[derive(Default)]
pub struct Registry {
    chains: HashMap<TypeId, Box<dyn Any>>,
}

impl Registry {
    fn chain_mut<O: Operation>(&mut self) -> &mut HandlerChain<O> {
        self.chains
            .entry(TypeId::of::<O>())
            .or_insert_with(|| Box::<HandlerChain<O>>::default())
            .downcast_mut::<HandlerChain<O>>()
            .expect("an operation TypeId uniquely determines its handler-chain type")
    }

    fn chain<O: Operation>(&self) -> Result<&HandlerChain<O>> {
        self.chains
            .get(&TypeId::of::<O>())
            .and_then(|chain| chain.downcast_ref::<HandlerChain<O>>())
            .ok_or_else(|| {
                syn::Error::new(
                    Span::call_site(),
                    format!("compiler operation `{}` is not registered", O::NAME),
                )
            })
    }

    /// Define the terminal implementation of an operation.
    ///
    /// Layers may be installed before or after the terminal is defined.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic if a terminal implementation is already present.
    pub fn define<O, F>(&mut self, terminal: F) -> Result<()>
    where
        O: Operation,
        F: Fn(&mut CompilerContext, O::Input) -> Result<O::Output> + 'static,
    {
        let chain = self.chain_mut::<O>();
        if chain.terminal.is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "compiler operation `{}` has more than one terminal implementation",
                    O::NAME
                ),
            ));
        }
        chain.terminal = Some(Box::new(terminal));
        Ok(())
    }

    /// Install an around-handler for an operation.
    ///
    /// A newly installed handler wraps handlers already present. Each handler
    /// receives an affine continuation for the remainder of the same
    /// operation.
    pub fn around<O, F>(&mut self, handler: F)
    where
        O: Operation,
        F: for<'a> Fn(&mut CompilerContext, O::Input, Next<'a, O>) -> Result<O::Output> + 'static,
    {
        self.chain_mut::<O>().around.insert(0, Box::new(handler));
    }

    /// Perform a registered compiler operation.
    ///
    /// # Errors
    ///
    /// Returns a registry diagnostic when the operation is absent or has no
    /// terminal implementation, or any diagnostic produced by its handlers.
    pub fn perform<O: Operation>(
        &self,
        context: &mut CompilerContext,
        input: O::Input,
    ) -> Result<O::Output> {
        Next {
            chain: self.chain::<O>()?,
            index: 0,
        }
        .call(context, input)
    }
}

/// An independently packaged compiler extension.
pub trait Layer {
    /// Install the layer's handlers and terminal implementations.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic when the layer conflicts with an existing
    /// operation definition.
    fn install(&self, registry: &mut Registry) -> Result<()>;
}

#[cfg(test)]
#[path = "../tests/unit/compiler.rs"]
mod tests;
