//! Helpers that build reader forms for the front-end integration tests.
//!
//! The reader lane has not landed, so forms are built directly from
//! `ncl-object` values. The fixture allocates conses without rooting them
//! across later allocations, which is safe here because the tests never
//! trigger a collection.
#![allow(clippy::expect_used, clippy::unwrap_used, dead_code)]

use ncl_compiler_front::{
    Declaration, Expr, FormExpander, FrontError, LambdaList, LambdaListKind, MacroCaller,
    MacroRegistry,
};
use ncl_object::{Package, Runtime, ThreadContext, Word, make_cons, make_string};

/// A runtime plus a registered context.
///
/// `ctx` is declared first so it drops before `runtime`: a registered context
/// dereferences its runtime's heap when it unregisters, so the runtime must
/// outlive it.
pub struct Fixture {
    /// The registered object-layer context.
    pub ctx: ThreadContext,
    /// The shared runtime.
    pub runtime: Runtime,
}

impl Fixture {
    /// Create a fixture with an empty heap.
    #[must_use]
    pub fn new() -> Self {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context");
        Self { ctx, runtime }
    }

    /// Intern `name` in `package` and return the symbol word.
    pub fn intern(&mut self, package: &str, name: &str) -> Word {
        let package = self
            .runtime
            .ensure_package(&mut self.ctx, package)
            .expect("package");
        Package::from(package)
            .intern(&mut self.ctx, &self.runtime, name)
            .expect("intern")
            .0
    }

    /// Intern a `COMMON-LISP` symbol.
    pub fn cl(&mut self, name: &str) -> Word {
        self.intern("COMMON-LISP", name)
    }

    /// Intern a `COMMON-LISP-USER` symbol.
    pub fn user(&mut self, name: &str) -> Word {
        self.intern("COMMON-LISP-USER", name)
    }

    /// Intern a `KEYWORD` symbol.
    pub fn keyword(&mut self, name: &str) -> Word {
        self.intern("KEYWORD", name)
    }

    /// Build a proper list.
    pub fn list(&mut self, elements: &[Word]) -> Word {
        let mut result = Word::NIL;
        for element in elements.iter().rev() {
            result = make_cons(&mut self.ctx, &self.runtime, *element, result).expect("cons");
        }
        result
    }

    /// Build `(operator argument*)` whose operator is a `COMMON-LISP` symbol.
    pub fn form(&mut self, operator: &str, arguments: &[Word]) -> Word {
        let head = self.cl(operator);
        let mut elements = vec![head];
        elements.extend_from_slice(arguments);
        self.list(&elements)
    }

    /// Build a string object.
    pub fn string(&mut self, text: &str) -> Word {
        make_string(
            &mut self.ctx,
            &self.runtime,
            &text.chars().collect::<Vec<char>>(),
        )
        .expect("string")
    }

    /// Expand one form with an empty compiler-macro registry.
    pub fn expand(&mut self, form: Word) -> Result<Expr, FrontError> {
        let registry = MacroRegistry::new();
        let mut expander = FormExpander::new(&mut self.ctx, &self.runtime, &registry);
        expander.expand(form)
    }

    /// Expand one form with a macro caller installed.
    pub fn expand_with(
        &mut self,
        form: Word,
        caller: &mut dyn MacroCaller,
    ) -> Result<Expr, FrontError> {
        let registry = MacroRegistry::new();
        let mut expander = FormExpander::new(&mut self.ctx, &self.runtime, &registry);
        expander.set_caller(caller);
        expander.expand(form)
    }

    /// Parse a `(declare ...)` form.
    pub fn declarations(&mut self, form: Word) -> Result<Vec<Declaration>, FrontError> {
        let registry = MacroRegistry::new();
        let mut expander = FormExpander::new(&mut self.ctx, &self.runtime, &registry);
        expander.declarations(form)
    }

    /// Parse one lambda list.
    pub fn lambda_list(
        &mut self,
        form: Word,
        kind: LambdaListKind,
    ) -> Result<LambdaList, FrontError> {
        let registry = MacroRegistry::new();
        let mut expander = FormExpander::new(&mut self.ctx, &self.runtime, &registry);
        expander.expand_lambda_list(form, kind)
    }
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

/// Intern `name` in `package` using an explicit context.
pub fn intern(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).expect("package");
    Package::from(package)
        .intern(ctx, runtime, name)
        .expect("intern")
        .0
}

/// Build a proper list using an explicit context.
pub fn list(ctx: &mut ThreadContext, runtime: &Runtime, elements: &[Word]) -> Word {
    let mut result = Word::NIL;
    for element in elements.iter().rev() {
        result = make_cons(ctx, runtime, *element, result).expect("cons");
    }
    result
}
