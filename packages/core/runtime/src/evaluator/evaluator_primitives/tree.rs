use super::*;

struct TreeEqualOptions {
    test: Option<Value>,
    test_not: Option<Value>,
    key: Option<Value>,
}

impl Runtime {
    pub(super) fn apply_tree_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "TREE-EQUAL" if arguments.len() >= 2 => self.apply_tree_equal(
                &arguments[0],
                &arguments[1],
                &arguments[2..],
                environment,
                span,
            ),
            "TREE-EQUAL" => Err(Self::arity("tree-equal", "at least two", arguments.len())),
            _ => return None,
        })
    }

    pub(crate) fn apply_tree_equal(
        &self,
        left: &Value,
        right: &Value,
        options: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let options = Self::parse_tree_equal_options(options, span)?;
        let default_test = options.test.is_none() && options.test_not.is_none();
        let invert_test = options.test_not.is_some();
        let test_designator = options
            .test
            .or(options.test_not)
            .unwrap_or_else(|| Value::symbol("EQUAL"));
        let test_function = Value::Function(self.resolve_function_designator(
            &test_designator,
            span,
            environment,
        )?);
        let key_function = options
            .key
            .map(|key| self.resolve_function_designator(&key, span, environment))
            .transpose()?
            .map(Value::Function);
        Ok(Value::Boolean(self.tree_equal_values(
            left,
            right,
            &test_function,
            invert_test,
            default_test,
            key_function.as_ref(),
            environment,
            span,
        )?))
    }

    fn tree_equal_values(
        &self,
        left: &Value,
        right: &Value,
        test: &Value,
        invert_test: bool,
        default_test: bool,
        key: Option<&Value>,
        environment: &Environment,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        match (left.list_items(), right.list_items()) {
            (Some(left_items), Some(right_items)) => {
                if left_items.len() != right_items.len() {
                    return Ok(false);
                }
                for (left_item, right_item) in left_items.iter().zip(&right_items) {
                    if !self.tree_equal_values(
                        left_item,
                        right_item,
                        test,
                        invert_test,
                        default_test,
                        key,
                        environment,
                        span,
                    )? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Some(_), None) | (None, Some(_)) => Ok(false),
            (None, None) if default_test => {
                let (left, right) =
                    self.apply_tree_equal_key(left, right, key, environment, span)?;
                Ok(left.equal_value(&right))
            }
            (None, None) => {
                let (left, right) =
                    self.apply_tree_equal_key(left, right, key, environment, span)?;
                let matches = self
                    .apply_in(test, &[left, right], span, environment)?
                    .primary_value()
                    .is_truthy();
                Ok(matches != invert_test)
            }
        }
    }

    fn apply_tree_equal_key(
        &self,
        left: &Value,
        right: &Value,
        key: Option<&Value>,
        environment: &Environment,
        span: Span,
    ) -> Result<(Value, Value), RuntimeError> {
        match key {
            Some(key) => Ok((
                self.apply_in(key, std::slice::from_ref(left), span, environment)?
                    .primary_value(),
                self.apply_in(key, std::slice::from_ref(right), span, environment)?
                    .primary_value(),
            )),
            None => Ok((left.clone(), right.clone())),
        }
    }

    fn parse_tree_equal_options(
        options: &[Value],
        span: Span,
    ) -> Result<TreeEqualOptions, RuntimeError> {
        if !options.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "tree-equal keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut parsed = TreeEqualOptions {
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
                        "tree-equal keyword argument name must be a keyword",
                        span,
                    ));
                }
            };
            match keyword_name.as_str() {
                "TEST" => {
                    if parsed.test_not.is_some() {
                        return Err(Self::invalid(
                            "tree-equal cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    parsed.test = Some(pair[1].clone());
                }
                "TEST-NOT" => {
                    if parsed.test.is_some() {
                        return Err(Self::invalid(
                            "tree-equal cannot use both :test and :test-not",
                            span,
                        ));
                    }
                    parsed.test_not = Some(pair[1].clone());
                }
                "KEY" => {
                    parsed.key = Some(pair[1].clone());
                }
                _ => {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown tree-equal keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }
        Ok(parsed)
    }
}
