#![allow(clippy::wildcard_imports)]
use super::*;

mod parameters;

use crate::value::MethodCombination;
use parameters::DefmethodParameters;

impl Runtime {
    pub(crate) fn special_defgeneric(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "defgeneric",
                "three",
                items.len().saturating_sub(1),
            ));
        }
        let name = Self::variable_name(&items[1], "defgeneric name must be a symbol")?;
        let name = unqualified_name(&name);
        let _ = Self::parameters(&items[2])?;
        let mut method_combination = MethodCombination::Standard;
        for option in items.iter().skip(3) {
            let option_items = Self::list_form_items(option, "defgeneric option")?;
            if option_items.is_empty() {
                return Err(Self::invalid(
                    "defgeneric option must be a non-empty list",
                    option.span,
                ));
            }
            let option_name =
                Self::definition_name_from_form(&option_items[0], "defgeneric option name")?;
            match option_name.as_str() {
                "DOCUMENTATION"
                    if option_items.len() != 2
                        || !matches!(option_items[1].kind, FormKind::String(_)) =>
                {
                    return Err(Self::invalid(
                        "defgeneric :documentation needs one string",
                        option.span,
                    ));
                }
                "DOCUMENTATION" => {}
                "METHOD-COMBINATION" if option_items.len() == 2 => {
                    method_combination = match Self::definition_name_from_form(
                        &option_items[1],
                        "defgeneric method combination",
                    )?
                    .as_str()
                    {
                        "STANDARD" => MethodCombination::Standard,
                        "AND" => MethodCombination::And,
                        "OR" => MethodCombination::Or,
                        "PROGN" => MethodCombination::Progn,
                        "LIST" => MethodCombination::List,
                        "APPEND" => MethodCombination::Append,
                        "NCONC" => MethodCombination::Nconc,
                        "+" => MethodCombination::Plus,
                        "MAX" => MethodCombination::Max,
                        "MIN" => MethodCombination::Min,
                        _ => {
                            return Err(Self::invalid(
                                "unsupported defgeneric method combination",
                                option.span,
                            ));
                        }
                    };
                }
                "METHOD-COMBINATION" => {
                    return Err(Self::invalid(
                        "unsupported defgeneric method combination",
                        option.span,
                    ));
                }
                _ => {
                    return Err(Self::invalid("unsupported defgeneric option", option.span));
                }
            }
        }
        environment.define_function(
            &name,
            Value::generic_with_lambda_list(name.clone(), items[2].clone(), method_combination),
        );
        Ok(Value::symbol(name))
    }

    pub(crate) fn special_defmethod(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "defmethod",
                "three",
                items.len().saturating_sub(1),
            ));
        }
        let name = Self::variable_name(&items[1], "defmethod name must be a symbol")?;
        let name = unqualified_name(&name);
        let lambda_index = items[2..]
            .iter()
            .position(|form| matches!(form.kind, FormKind::List(_)))
            .map(|index| index + 2)
            .ok_or_else(|| {
                Self::invalid("defmethod requires a method lambda list", items[1].span)
            })?;

        let qualifiers = items[2..lambda_index]
            .iter()
            .map(|form| {
                let qualifier = Self::definition_name_from_form(form, "defmethod qualifier")?;
                match qualifier.as_str() {
                    "BEFORE" | "AFTER" | "AROUND" => Ok(qualifier),
                    _ => Err(Self::invalid("unsupported defmethod qualifier", form.span)),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        if qualifiers.len() > 1 {
            return Err(Self::invalid(
                "defmethod accepts at most one method qualifier",
                items[2].span,
            ));
        }
        let FormKind::List(parameters) = &items[lambda_index].kind else {
            return Err(Self::invalid(
                "defmethod lambda list must be a list",
                items[lambda_index].span,
            ));
        };

        let DefmethodParameters {
            required,
            required_escaped,
            specializers,
            mut normalized,
            required_count,
        } = self.parse_defmethod_required_parameters(parameters, environment)?;
        normalized.extend(
            parameters
                .get(required_count..)
                .unwrap_or_default()
                .iter()
                .cloned(),
        );
        let normalized_lambda_list = Form::list(normalized, items[lambda_index].span);
        let lambda_list = Self::parameters(&normalized_lambda_list)?;

        let generic = environment.lookup_function(&name).or_else(|| {
            let generic = Value::generic(name.clone());
            environment.define_function(&name, generic.clone());
            Some(generic)
        });
        let Some(Value::Function(generic)) = generic else {
            return Err(Self::invalid(
                "defmethod name is not a generic function",
                items[1].span,
            ));
        };
        let crate::Function::Generic { methods, .. } = generic.as_ref() else {
            return Err(Self::invalid(
                "defmethod name is not a generic function",
                items[1].span,
            ));
        };
        let closure = Value::closure_with_keywords(
            crate::ClosureOptions {
                parameters: required,
                required_escaped,
                optional: lambda_list.optional,
                rest: lambda_list.rest,
                rest_escaped: lambda_list.rest_escaped,
                keywords: lambda_list.keywords,
                has_keyword_section: lambda_list.has_keyword_section,
                allow_other_keys: lambda_list.allow_other_keys,
                auxiliary: lambda_list.auxiliary,
            },
            items[lambda_index + 1..].to_vec(),
            environment.clone(),
        );
        methods.borrow_mut().push(MethodDefinition {
            qualifiers,
            specializers,
            function: closure,
        });
        Ok(Value::symbol(name))
    }
}
