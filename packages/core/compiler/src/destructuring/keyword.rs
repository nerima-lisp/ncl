#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(super) fn compile_destructuring_keyword_name(form: &Form) -> Result<String, CompileError> {
        let FormKind::Atom(name) = &form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "destructuring keyword name".to_string(),
                },
                form.span,
            ));
        };
        let Some(keyword) = name.strip_prefix(':') else {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword designator must start with a keyword"
                        .to_string(),
                },
                form.span,
            ));
        };
        if keyword.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword designator must be nonempty".to_string(),
                },
                form.span,
            ));
        }
        Ok(normalize_name(keyword))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn compile_destructuring_keyword_parameter(
        &mut self,
        form: &Form,
        seen: &mut HashSet<String>,
    ) -> Result<DestructureKeywordParameter, CompileError> {
        let nil = || Form::atom("NIL", form.span);
        let (keyword_name, pattern, trailing_start) = match &form.kind {
            FormKind::Atom(_) => {
                let name = Self::compile_destructuring_binding_name(
                    form,
                    seen,
                    "destructuring keyword parameter name",
                )?;
                let keyword_name = normalize_name(&name);
                (keyword_name, DestructurePattern::Name(name), 0)
            }
            FormKind::List(items) if !items.is_empty() => {
                if let FormKind::List(key_specification) = &items[0].kind {
                    if key_specification.len() != 2 {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "destructuring keyword designator must contain a keyword and variable"
                                    .to_string(),
                            },
                            items[0].span,
                        ));
                    }
                    let keyword_name =
                        Self::compile_destructuring_keyword_name(&key_specification[0])?;
                    let pattern = Self::compile_destructuring_pattern(&key_specification[1], seen)?;
                    (keyword_name, pattern, 1)
                } else if matches!(&items[0].kind, FormKind::Atom(name) if name.starts_with(':')) {
                    let keyword_name = Self::compile_destructuring_keyword_name(&items[0])?;
                    if items.len() < 2 {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message: "destructuring keyword parameter needs a variable"
                                    .to_string(),
                            },
                            form.span,
                        ));
                    }
                    let pattern = Self::compile_destructuring_pattern(&items[1], seen)?;
                    (keyword_name, pattern, 2)
                } else {
                    let pattern = Self::compile_destructuring_pattern(&items[0], seen)?;
                    let DestructurePattern::Name(name) = &pattern else {
                        return Err(CompileError::new(
                            CompileErrorKind::InvalidForm {
                                message:
                                    "destructuring keyword parameter must have a variable name"
                                        .to_string(),
                            },
                            items[0].span,
                        ));
                    };
                    (normalize_name(name), pattern, 1)
                }
            }
            FormKind::List(_) => unreachable!(),
            _ => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidForm {
                        message: "destructuring keyword parameter must be a symbol or list"
                            .to_string(),
                    },
                    form.span,
                ));
            }
        };

        let item_count = match &form.kind {
            FormKind::Atom(_) => 0,
            FormKind::List(items) => items.len(),
            _ => unreachable!(),
        };
        if item_count > trailing_start + 2 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "destructuring keyword parameter contains too many items".to_string(),
                },
                form.span,
            ));
        }
        let (init_form, supplied_p) = match &form.kind {
            FormKind::Atom(_) => (nil(), None),
            FormKind::List(items) => (
                items.get(trailing_start).cloned().unwrap_or_else(nil),
                items
                    .get(trailing_start + 1)
                    .map(|item| {
                        Self::compile_destructuring_binding_name(
                            item,
                            seen,
                            "destructuring supplied-p name",
                        )
                    })
                    .transpose()?,
            ),
            _ => unreachable!(),
        };
        let default_function = self.compile_destructuring_default(&init_form)?;
        Ok(DestructureKeywordParameter {
            keyword_name,
            pattern,
            default_function,
            supplied_p,
        })
    }
}
