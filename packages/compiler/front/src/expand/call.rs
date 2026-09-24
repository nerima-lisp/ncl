//! Macroexpansion steps of the form expander.
//!
//! These methods resolve one form to an [`Expr`] or to a replacement that must
//! be expanded again. They live apart from `expand` to keep both files small.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, car, symbol_is_macro};

use crate::ast::{Expr, LocalMacro, Operator};
use crate::error::FrontError;
use crate::expand::{FormExpander, SYMBOL_MACRO_LIMIT};
use crate::form::classify_form;
use crate::literal::Literal;
use crate::macro_caller::MacroCaller;
use crate::special::{self, SpecialForm};
use crate::symbols::SymbolRef;

/// The result of one expansion step.
pub(super) enum Step {
    /// The form is fully expanded.
    Done(Expr),
    /// The form was macroexpanded and must be expanded again.
    Retry {
        /// The macro that produced the replacement.
        name: SymbolRef,
        /// The replacement form.
        form: Word,
    },
}

impl<'a> FormExpander<'a> {
    /// Expand one step of a form.
    pub(super) fn expand_step(&mut self, form: Word) -> Result<Step, FrontError> {
        match classify_form(self.ctx, form) {
            ObjectRef::Symbol(_) => self.expand_symbol(form),
            ObjectRef::Cons(_) => self.expand_cons(form),
            _ => {
                let datum = self.datum(form)?;
                Ok(Step::Done(Expr::Constant(datum)))
            }
        }
    }

    /// Expand a symbol in value position.
    fn expand_symbol(&mut self, word: Word) -> Result<Step, FrontError> {
        if word == Word::NIL {
            return Ok(Step::Done(Expr::Constant(Literal::Nil)));
        }
        let name = self.symbol(word)?;
        if let Some(expansion) = self.symbol_macro_expansion(&name) {
            return Ok(Step::Done(expansion));
        }
        if name.is_named("COMMON-LISP", "NIL") {
            return Ok(Step::Done(Expr::Constant(Literal::Nil)));
        }
        if name.is_named("COMMON-LISP", "T") {
            return Ok(Step::Done(Expr::Constant(Literal::T)));
        }
        if name.is_keyword() {
            return Ok(Step::Done(Expr::Constant(Literal::Symbol(name))));
        }
        Ok(Step::Done(Expr::Variable(name)))
    }

    /// Resolve a symbol macro, following a chain of symbol macros.
    ///
    /// A symbol macro expands to the form recorded by `symbol-macrolet`; when
    /// that form is itself a bare reference to another symbol macro, the chain
    /// is followed so `(symbol-macrolet ((x y) (y 1)) x)` resolves to `1`.
    fn symbol_macro_expansion(&self, name: &SymbolRef) -> Option<Expr> {
        let mut expansion = self.env.lookup_symbol_macro(name)?.expansion.clone();
        let mut remaining = SYMBOL_MACRO_LIMIT;
        while remaining > 0 {
            let Expr::Variable(inner) = &expansion else {
                break;
            };
            let Some(next) = self.env.lookup_symbol_macro(inner) else {
                break;
            };
            expansion = next.expansion.clone();
            remaining -= 1;
        }
        Some(expansion)
    }

    /// Expand a cons form.
    fn expand_cons(&mut self, form: Word) -> Result<Step, FrontError> {
        let elements = self.elements(form)?;
        let Some((head, arguments)) = elements.split_first() else {
            return Ok(Step::Done(Expr::Constant(Literal::Nil)));
        };
        if head.is_cons() {
            return self.expand_lambda_call(*head, arguments);
        }
        if !matches!(classify_form(self.ctx, *head), ObjectRef::Symbol(_)) {
            return Err(FrontError::InvalidOperator {
                detail: "operator is not a symbol".to_owned(),
            });
        }
        let name = self.symbol(*head)?;
        if let Some(kind) = SpecialForm::from_symbol(&name) {
            return Ok(Step::Done(special::parse(self, kind, form)?));
        }
        if let Some(definition) = self.env.lookup_macro(&name) {
            let definition = definition.clone();
            let replacement = Self::call_local_macro(
                &mut self.caller,
                self.ctx,
                self.runtime,
                &definition,
                form,
            )?;
            return Ok(Step::Retry {
                name,
                form: replacement,
            });
        }
        if symbol_is_macro(self.ctx, *head)? {
            let replacement =
                Self::call_global_macro(&mut self.caller, self.ctx, self.runtime, &name, form)?;
            return Ok(Step::Retry {
                name,
                form: replacement,
            });
        }
        let registry = self.registry;
        if let Some(entry) = registry.lookup(&name)
            && entry.arity.accepts(arguments.len())
            && let Some(replacement) = (entry.expander)(self.ctx, self.runtime, form)?
        {
            return Ok(Step::Retry {
                name,
                form: replacement,
            });
        }
        let arguments = self.expand_all(arguments)?;
        Ok(Step::Done(Expr::Call {
            operator: Operator::Name(name),
            arguments,
        }))
    }

    /// Expand a call whose operator is a lambda expression.
    fn expand_lambda_call(&mut self, head: Word, arguments: &[Word]) -> Result<Step, FrontError> {
        let head_symbol = car(self.ctx, head)?;
        if !self.is_named(head_symbol, "LAMBDA")? {
            return Err(FrontError::InvalidOperator {
                detail: "operator is neither a symbol nor a lambda expression".to_owned(),
            });
        }
        let lambda = self.expand_lambda(head)?;
        let arguments = self.expand_all(arguments)?;
        Ok(Step::Done(Expr::Call {
            operator: Operator::Lambda(Box::new(lambda)),
            arguments,
        }))
    }

    /// Call a global macro through the installed caller.
    fn call_global_macro(
        caller: &mut Option<&'a mut dyn MacroCaller>,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &SymbolRef,
        form: Word,
    ) -> Result<Word, FrontError> {
        caller.as_mut().map_or_else(
            || {
                Err(FrontError::MacroExpansion {
                    name: name.clone(),
                    detail: "no macro caller is installed".to_owned(),
                })
            },
            |caller| caller.call_macro(ctx, runtime, name, form),
        )
    }

    /// Call a local macro through the installed caller.
    fn call_local_macro(
        caller: &mut Option<&'a mut dyn MacroCaller>,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        definition: &LocalMacro,
        form: Word,
    ) -> Result<Word, FrontError> {
        caller.as_mut().map_or_else(
            || {
                Err(FrontError::MacroExpansion {
                    name: definition.name.clone(),
                    detail: "no macro caller is installed".to_owned(),
                })
            },
            |caller| caller.call_local_macro(ctx, runtime, definition, form),
        )
    }
}
