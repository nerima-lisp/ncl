//! Construction of semantic HIR from the frozen front-end AST.

mod bindings;
mod calls;
mod context;
mod control;
mod declarations;
mod error;
mod expr;
mod literals;
mod types;

pub use error::{HirBuildError, UnsupportedHirForm};

use ncl_hir::{CompilationUnitId, FormId, HirForm, SourceOriginId, SourceOrigins, SymbolTable};

/// Builds one top-level HIR form at a time while interning symbols per unit.
#[derive(Debug)]
pub struct HirBuilder {
    symbols: SymbolTable,
}

impl HirBuilder {
    /// Creates a bridge for one compilation unit.
    #[must_use]
    pub fn new(compilation_unit: CompilationUnitId) -> Self {
        Self {
            symbols: SymbolTable::new(compilation_unit),
        }
    }

    /// Returns the compilation-unit symbol table shared by built forms.
    #[must_use]
    pub const fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    /// Builds an HIR form using caller-supplied source origins.
    ///
    /// Each AST node uses `default_origin` because the frozen front AST has no
    /// node-level span. The bridge does not infer or manufacture source ranges.
    ///
    /// # Errors
    ///
    /// Returns [`HirBuildError`] when the AST uses a form the current HIR schema
    /// cannot represent, or when an identity or scope cannot be constructed.
    pub fn build_form(
        &mut self,
        form: FormId,
        expr: &crate::ast::Expr,
        origins: SourceOrigins,
        default_origin: SourceOriginId,
    ) -> Result<HirForm, HirBuildError> {
        let mut context =
            context::BuildContext::new(form, origins, default_origin, &mut self.symbols)?;
        let root_scope = context.form.root_scope();
        let root = context.lower_expr(expr, root_scope)?;
        context.form.add_root(root)?;
        let form = context.finish()?;
        form.verify(&self.symbols)
            .map_err(HirBuildError::Verification)?;
        Ok(form)
    }
}
