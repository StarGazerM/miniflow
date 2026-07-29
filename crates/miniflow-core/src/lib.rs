//! Syntax-independent compiler kernel for `MiniFlow`.
//!
//! This crate is intentionally usable outside a procedural macro so expansion
//! parity tests exercise exactly the compiler invoked by the public macros.

mod canonical;
pub mod compiler;
mod dd_render;
mod flowlog_analysis;
mod flowlog_fp;
pub mod flowlog_plan;
pub mod hir;
mod pipeline;
pub mod plan;
pub mod program_plan;
pub mod rule_plan;
pub mod scc_plan;
pub mod source;

use syn::Result;

pub use canonical::extract_dataflow_core;
pub use hir::HirProgram;
pub use pipeline::{
    Compiler, PlanRule, PlanScc, PlanningCatalog, ReadSource, RuleRequest, SccRequest,
};
pub use source::Program;
pub use source::Program as SourceProgram;

/// Lower a parsed program into relation-identified HIR and dependency SCCs.
///
/// # Errors
///
/// Returns a semantic error for invalid attributes, relation references, or
/// arities.
pub fn lower(program: Program) -> Result<HirProgram> {
    HirProgram::lower(program)
}
