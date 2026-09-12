use std::collections::{HashMap, HashSet};

use super::*;

struct TreeSubstitutionOptions {
    test: Option<Value>,
    test_not: Option<Value>,
    key: Option<Value>,
}

struct TreeSubstitutionMatcher {
    function: Value,
    key: Option<Value>,
    invert: bool,
    unary: bool,
}

impl Runtime {
    pub(super) fn apply_tree_substitution_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "SUBST" | "NSUBST" if arguments.len() >= 3 => self.apply_subst(
                name,
                &arguments[0],
                &arguments[1],
                &arguments[2],
                &arguments[3..],
                environment,
                span,
            ),
            "SUBST" | "NSUBST" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least three",
                arguments.len(),
            )),
            "SUBST-IF" | "SUBST-IF-NOT" | "NSUBST-IF" | "NSUBST-IF-NOT" if arguments.len() >= 3 => {
                self.apply_subst_if(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2],
                    &arguments[3..],
                    environment,
                    span,
                )
            }
            "SUBST-IF" | "SUBST-IF-NOT" | "NSUBST-IF" | "NSUBST-IF-NOT" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least three",
                arguments.len(),
            )),
            "SUBLIS" | "NSUBLIS" if arguments.len() >= 2 => self.apply_sublis(
                name,
                &arguments[0],
                &arguments[1],
                &arguments[2..],
                environment,
                span,
            ),
            "SUBLIS" | "NSUBLIS" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least two",
                arguments.len(),
            )),
            _ => return None,
        })
    }

    fn apply_subst(
        &self,
        name: &str,
        new_value: &Value,
        old_value: &Value,
        tree: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let parsed = Self::parse_tree_substitution_options(options, true, name, span)?;
        let matcher = self.resolve_tree_substitution_matcher(parsed, false, environment, span)?;
        if name.starts_with('N') {
            let mut seen = HashSet::new();
            self.substitute_tree_destructively(
                tree,
                Some(old_value),
                new_value,
                &matcher,
                &mut seen,
                environment,
                span,
            )
        } else {
            let mut copies = HashMap::new();
            self.substitute_tree_copy(
                tree,
                Some(old_value),
                new_value,
                &matcher,
                &mut copies,
                environment,
                span,
            )
        }
    }

    fn apply_subst_if(
        &self,
        name: &str,
        new_value: &Value,
        predicate: &Value,
        tree: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let parsed = Self::parse_tree_substitution_options(options, false, name, span)?;
        let matcher = self.resolve_tree_substitution_matcher(
            parsed,
            name.ends_with("-IF-NOT"),
            environment,
            span,
        )?;
        let matcher = TreeSubstitutionMatcher {
            function: self
                .resolve_function_designator(predicate, span, environment)
                .map(Value::Function)?,
            key: matcher.key,
            invert: matcher.invert,
            unary: true,
        };
        if name.starts_with('N') {
            let mut seen = HashSet::new();
            self.substitute_tree_destructively(
                tree,
                None,
                new_value,
                &matcher,
                &mut seen,
                environment,
                span,
            )
        } else {
            let mut copies = HashMap::new();
            self.substitute_tree_copy(
                tree,
                None,
                new_value,
                &matcher,
                &mut copies,
                environment,
                span,
            )
        }
    }

    fn apply_sublis(
        &self,
        name: &str,
        alist: &Value,
        tree: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let entries = alist.list_items().ok_or_else(|| RuntimeError::Type {
            expected: "ALIST".to_owned(),
            actual: alist.type_name().to_owned(),
            span: Some(span),
        })?;
        let entries = entries
            .into_iter()
            .map(|entry| match entry {
                Value::Cons(cell) => Ok((cell.car(), cell.cdr())),
                Value::Nil | Value::Boolean(false) => Err(Self::invalid(
                    "sublis association list entries must be conses",
                    span,
                )),
                value => Err(Self::invalid(
                    &format!("sublis association list entry is not a cons: {value}"),
                    span,
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let parsed = Self::parse_tree_substitution_options(options, true, name, span)?;
        let matcher = self.resolve_tree_substitution_matcher(parsed, false, environment, span)?;
        if name.starts_with('N') {
            let mut seen = HashSet::new();
            self.sublis_tree_destructively(tree, &entries, &matcher, &mut seen, environment, span)
        } else {
            let mut copies = HashMap::new();
            self.sublis_tree_copy(tree, &entries, &matcher, &mut copies, environment, span)
        }
    }

    fn parse_tree_substitution_options(
        options: &[Value],
        allow_test: bool,
        operation: &str,
        span: Span,
    ) -> Result<TreeSubstitutionOptions, RuntimeError> {
        if !options.len().is_multiple_of(2) {
            return Err(Self::invalid(
                &format!("{operation} keyword arguments must be supplied in pairs"),
                span,
            ));
        }
        let mut parsed = TreeSubstitutionOptions {
            test: None,
            test_not: None,
            key: None,
        };
        for pair in options.as_chunks::<2>().0 {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                Value::InternedSymbol(symbol) if symbol.keyword() => normalize_name(symbol.name()),
                _ => {
                    return Err(Self::invalid(
                        &format!("{operation} keyword argument name must be a keyword"),
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "TEST" if allow_test => {
                    if parsed.test_not.is_some() {
                        return Err(Self::invalid(
                            &format!("{operation} cannot use both :test and :test-not"),
                            span,
                        ));
                    }
                    parsed.test = Some(pair[1].clone());
                }
                "TEST-NOT" if allow_test => {
                    if parsed.test.is_some() {
                        return Err(Self::invalid(
                            &format!("{operation} cannot use both :test and :test-not"),
                            span,
                        ));
                    }
                    parsed.test_not = Some(pair[1].clone());
                }
                "KEY" => parsed.key = Some(pair[1].clone()),
                "TEST" | "TEST-NOT" => {
                    return Err(Self::invalid(
                        &format!("{operation} does not accept :{keyword_name}"),
                        span,
                    ));
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown {operation} keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }
        Ok(parsed)
    }

    fn resolve_tree_substitution_matcher(
        &self,
        options: TreeSubstitutionOptions,
        invert: bool,
        environment: &Environment,
        span: Span,
    ) -> Result<TreeSubstitutionMatcher, RuntimeError> {
        let invert = invert || options.test_not.is_some();
        let designator = options
            .test
            .or(options.test_not)
            .unwrap_or_else(|| Value::symbol("EQL"));
        Ok(TreeSubstitutionMatcher {
            function: Value::Function(self.resolve_function_designator(
                &designator,
                span,
                environment,
            )?),
            key: match options.key {
                Some(key) if key.is_truthy() => Some(Value::Function(
                    self.resolve_function_designator(&key, span, environment)?,
                )),
                _ => None,
            },
            invert,
            unary: false,
        })
    }

    fn tree_substitution_match(
        &self,
        matcher: &TreeSubstitutionMatcher,
        expected: Option<&Value>,
        candidate: &Value,
        environment: &Environment,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        let candidate = match &matcher.key {
            Some(key) => self
                .apply_in(key, std::slice::from_ref(candidate), span, environment)?
                .primary_value(),
            None => candidate.clone(),
        };
        let arguments = match (matcher.unary, expected) {
            (true, _) => vec![candidate],
            (false, Some(expected)) => vec![expected.clone(), candidate],
            (false, None) => unreachable!("binary tree substitution matcher missing target"),
        };
        let matches = self
            .apply_in(&matcher.function, &arguments, span, environment)?
            .primary_value()
            .is_truthy();
        Ok(if matcher.invert { !matches } else { matches })
    }

    fn substitute_tree_copy(
        &self,
        value: &Value,
        expected: Option<&Value>,
        new_value: &Value,
        matcher: &TreeSubstitutionMatcher,
        copies: &mut HashMap<usize, Value>,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if self.tree_substitution_match(matcher, expected, value, environment, span)? {
            return Ok(new_value.clone());
        }
        let Value::Cons(cell) = value else {
            return Ok(value.clone());
        };
        if let Some(existing) = copies.get(&cell.identity()) {
            return Ok(existing.clone());
        }
        let result = Value::cons(Value::Nil, Value::Nil);
        copies.insert(cell.identity(), result.clone());
        let Value::Cons(target) = &result else {
            unreachable!();
        };
        target.set_car(self.substitute_tree_copy(
            &cell.car(),
            expected,
            new_value,
            matcher,
            copies,
            environment,
            span,
        )?);
        target.set_cdr(self.substitute_tree_copy(
            &cell.cdr(),
            expected,
            new_value,
            matcher,
            copies,
            environment,
            span,
        )?);
        Ok(result)
    }

    fn substitute_tree_destructively(
        &self,
        value: &Value,
        expected: Option<&Value>,
        new_value: &Value,
        matcher: &TreeSubstitutionMatcher,
        seen: &mut HashSet<usize>,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Value::Cons(cell) = value {
            if seen.contains(&cell.identity()) {
                return Ok(value.clone());
            }
        }
        if self.tree_substitution_match(matcher, expected, value, environment, span)? {
            return Ok(new_value.clone());
        }
        let Value::Cons(cell) = value else {
            return Ok(value.clone());
        };
        seen.insert(cell.identity());
        let car = self.substitute_tree_destructively(
            &cell.car(),
            expected,
            new_value,
            matcher,
            seen,
            environment,
            span,
        )?;
        let cdr = self.substitute_tree_destructively(
            &cell.cdr(),
            expected,
            new_value,
            matcher,
            seen,
            environment,
            span,
        )?;
        cell.set_car(car);
        cell.set_cdr(cdr);
        Ok(value.clone())
    }

    fn sublis_tree_copy(
        &self,
        value: &Value,
        entries: &[(Value, Value)],
        matcher: &TreeSubstitutionMatcher,
        copies: &mut HashMap<usize, Value>,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Some(replacement) =
            self.sublis_replacement(value, entries, matcher, environment, span)?
        {
            return Ok(replacement);
        }
        let Value::Cons(cell) = value else {
            return Ok(value.clone());
        };
        if let Some(existing) = copies.get(&cell.identity()) {
            return Ok(existing.clone());
        }
        let result = Value::cons(Value::Nil, Value::Nil);
        copies.insert(cell.identity(), result.clone());
        let Value::Cons(target) = &result else {
            unreachable!();
        };
        target.set_car(self.sublis_tree_copy(
            &cell.car(),
            entries,
            matcher,
            copies,
            environment,
            span,
        )?);
        target.set_cdr(self.sublis_tree_copy(
            &cell.cdr(),
            entries,
            matcher,
            copies,
            environment,
            span,
        )?);
        Ok(result)
    }

    fn sublis_tree_destructively(
        &self,
        value: &Value,
        entries: &[(Value, Value)],
        matcher: &TreeSubstitutionMatcher,
        seen: &mut HashSet<usize>,
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Value::Cons(cell) = value {
            if seen.contains(&cell.identity()) {
                return Ok(value.clone());
            }
        }
        if let Some(replacement) =
            self.sublis_replacement(value, entries, matcher, environment, span)?
        {
            return Ok(replacement);
        }
        let Value::Cons(cell) = value else {
            return Ok(value.clone());
        };
        seen.insert(cell.identity());
        let car =
            self.sublis_tree_destructively(&cell.car(), entries, matcher, seen, environment, span)?;
        let cdr =
            self.sublis_tree_destructively(&cell.cdr(), entries, matcher, seen, environment, span)?;
        cell.set_car(car);
        cell.set_cdr(cdr);
        Ok(value.clone())
    }

    fn sublis_replacement(
        &self,
        value: &Value,
        entries: &[(Value, Value)],
        matcher: &TreeSubstitutionMatcher,
        environment: &Environment,
        span: Span,
    ) -> Result<Option<Value>, RuntimeError> {
        for (key, replacement) in entries {
            if self.tree_substitution_match(matcher, Some(key), value, environment, span)? {
                return Ok(Some(replacement.clone()));
            }
        }
        Ok(None)
    }
}
