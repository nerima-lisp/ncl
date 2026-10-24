use ncl_object::{HeapRootedWord, ObjectRef, Runtime, ThreadContext, Word, make_cons};

use crate::declaration::{Declaration, parse_declare_form};
use crate::error::FrontError;
use crate::form::classify_form;

use super::FormExpander;

impl FormExpander<'_> {
    /// Parse a `(declare ...)` form after expanding runtime-known type names.
    ///
    /// # Errors
    ///
    /// Returns a [`FrontError`] when the form is not a well-formed declaration
    /// or a runtime type expander fails.
    pub fn declarations(&mut self, form: Word) -> Result<Vec<Declaration>, FrontError> {
        let form = HeapRootedWord::new(self.ctx, form)?;
        let elements = self.elements(form.get())?;
        let Some(operator) = elements.first() else {
            let datum = self.datum(form.get())?;
            return parse_declare_form(&datum);
        };

        let mut rewritten = Vec::with_capacity(elements.len().saturating_sub(1));
        let mut changed = false;
        for declaration in elements.iter().skip(1) {
            let declaration = HeapRootedWord::new(self.ctx, declaration.get())?;
            let declaration_elements = match self.elements(declaration.get()) {
                Ok(elements) => elements,
                Err(FrontError::ImproperList) => {
                    rewritten.push(declaration);
                    continue;
                }
                Err(error) => return Err(error),
            };
            let Some(specifier) = declaration_elements.first() else {
                rewritten.push(declaration);
                continue;
            };
            if !matches!(
                classify_form(self.ctx, specifier.get()),
                ObjectRef::Symbol(_)
            ) {
                rewritten.push(declaration);
                continue;
            }
            let specifier_name = self.symbol(specifier.get())?.name;
            if !matches!(specifier_name.as_str(), "TYPE" | "FTYPE")
                || declaration_elements.len() < 2
            {
                rewritten.push(declaration);
                continue;
            }

            let type_specifier = HeapRootedWord::new(
                self.ctx,
                declaration_elements
                    .get(1)
                    .ok_or(FrontError::ImproperList)?
                    .get(),
            )?;
            let expanded = self.expand_declared_type(type_specifier.get())?;
            let expanded = HeapRootedWord::new(self.ctx, expanded)?;
            if expanded.get() == type_specifier.get() {
                rewritten.push(declaration);
                continue;
            }

            let mut parts = declaration_elements
                .iter()
                .map(HeapRootedWord::get)
                .collect::<Vec<_>>();
            *parts.get_mut(1).ok_or(FrontError::ImproperList)? = expanded.get();
            let replacement = list_from_words(self.ctx, self.runtime, &parts)?;
            rewritten.push(HeapRootedWord::new(self.ctx, replacement)?);
            changed = true;
        }

        if !changed {
            let datum = self.datum(form.get())?;
            return parse_declare_form(&datum);
        }

        let mut words = Vec::with_capacity(rewritten.len() + 1);
        words.push(operator.get());
        words.extend(rewritten.iter().map(HeapRootedWord::get));
        let rewritten_form = list_from_words(self.ctx, self.runtime, &words)?;
        let rewritten_form = HeapRootedWord::new(self.ctx, rewritten_form)?;
        let datum = self.datum(rewritten_form.get())?;
        parse_declare_form(&datum)
    }

    fn expand_declared_type(&mut self, specifier: Word) -> Result<Word, FrontError> {
        if let Some(caller) = self.caller.as_deref_mut() {
            caller.expand_type_specifier(self.ctx, self.runtime, specifier)
        } else {
            Ok(specifier)
        }
    }
}

fn list_from_words(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    words: &[Word],
) -> Result<Word, FrontError> {
    ncl_object::with_roots(ctx, words, |ctx, words| {
        let mut result = Word::NIL;
        for word in words.iter().rev() {
            result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
                make_cons(ctx, runtime, **word, *result)
            })?;
        }
        Ok(result)
    })
    .map_err(Into::into)
}
