use super::{Environment, Form, FormKind, HashSet, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_flet(
        &self,
        items: &[Form],
        environment: &Environment,
        recursive: bool,
    ) -> Result<Value, RuntimeError> {
        let operator = if recursive { "labels" } else { "flet" };
        if items.len() < 2 {
            return Err(Self::arity(
                operator,
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(bindings) = &items[1].kind else {
            return Err(Self::invalid(
                "local function bindings must be a list",
                items[1].span,
            ));
        };

        let local = environment.child();
        let captured = if recursive {
            local.clone()
        } else {
            environment.clone()
        };
        let mut names = HashSet::new();
        let mut definitions = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let FormKind::List(parts) = &binding.kind else {
                return Err(Self::invalid(
                    "local function binding must be a list",
                    binding.span,
                ));
            };
            if parts.len() < 3 {
                return Err(Self::invalid(
                    "local function needs a name, parameters, and a body",
                    binding.span,
                ));
            }
            let (normalized, escaped) =
                Self::variable_name_info(&parts[0], "local function name must be a symbol")?;
            if !names.insert(normalized.clone()) {
                return Err(Self::invalid(
                    "local function names must be unique",
                    parts[0].span,
                ));
            }
            definitions.push((
                normalized,
                escaped,
                Self::parameters(&parts[1])?,
                parts[2..].to_vec(),
            ));
        }

        for (name, escaped, lambda_list, body) in definitions {
            let function = Value::closure_with_keywords(
                crate::ClosureOptions {
                    parameters: lambda_list.required,
                    required_escaped: lambda_list.required_escaped,
                    optional: lambda_list.optional,
                    rest: lambda_list.rest,
                    rest_escaped: lambda_list.rest_escaped,
                    keywords: lambda_list.keywords,
                    has_keyword_section: lambda_list.has_keyword_section,
                    allow_other_keys: lambda_list.allow_other_keys,
                    auxiliary: lambda_list.auxiliary,
                },
                body,
                captured.clone(),
            );
            if escaped {
                local.define_function_exact(name, function);
            } else {
                local.define_function(name, function);
            }
        }
        self.eval_sequence_values(&items[2..], &local)
    }
}
