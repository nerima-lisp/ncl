#![allow(clippy::wildcard_imports)]
use super::*;

mod clos_instance;
mod closure;
mod compile_load;
mod condition_dispatch_tests;
mod generic_dispatch;
mod generic_invocation;
mod sequence_mapping;
mod slot_access;
mod structure_boa;
mod structure_boa_apply;
mod structure_boa_binding_keywords;
mod structure_boa_binding_positional;
mod structure_construction;
use structure_construction::StructureConstructorContext;

struct ClosureApplicationContext<'a> {
    parameters: &'a [String],
    required_escaped: &'a [bool],
    optional: &'a [LambdaListOptionalParameter],
    rest: Option<&'a String>,
    rest_escaped: bool,
    keywords: &'a [LambdaListKeywordParameter],
    has_keyword_section: bool,
    allow_other_keys: bool,
    auxiliary: &'a [LambdaListAuxiliaryParameter],
    body: &'a [Form],
    environment: &'a Environment,
    arguments: &'a [Value],
    span: Span,
}

struct ClosureKeywordApplicationContext<'a> {
    keywords: &'a [LambdaListKeywordParameter],
    arguments: &'a [Value],
    key_start: usize,
    allow_other_keys: bool,
    local: &'a Environment,
    span: Span,
    special_names: &'a [(String, bool)],
}

impl Runtime {
    pub(crate) fn apply_in(
        &self,
        function: &Value,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let function = self.resolve_function_designator(function, span, environment)?;
        match function.as_ref() {
            crate::Function::Builtin { name, function } => {
                let random_state = self
                    .lookup_in("*random-state*", environment)
                    .and_then(|value| value.random_state_reference());
                let standard_input =
                    self.lookup_in("*standard-input*", environment)
                        .and_then(|value| match value {
                            Value::Stream(stream) => Some(stream),
                            _ => None,
                        });
                let standard_output = self.lookup_in("*standard-output*", environment).and_then(
                    |value| match value {
                        Value::Stream(stream) => Some(stream),
                        _ => None,
                    },
                );
                crate::builtins::with_random_state_context(random_state, || {
                    crate::builtins::with_stream_context(standard_input, standard_output, || {
                        match *name {
                            "typep" => {
                                if arguments.len() != 2 {
                                    return Err(Self::arity("typep", "two", arguments.len()));
                                }
                                crate::builtins::typep_value_in(
                                    &arguments[0],
                                    &arguments[1],
                                    environment,
                                )
                                .map(Value::boolean)
                            }
                            "read-from-string" => {
                                crate::builtins::read_from_string_in(self, arguments)
                            }
                            "read" => crate::builtins::read_in(self, arguments),
                            "read-preserving-whitespace" => {
                                crate::builtins::read_preserving_whitespace_in(self, arguments)
                            }
                            _ => function(arguments),
                        }
                    })
                })
            }
            crate::Function::Complement { function } => self
                .apply_in(function, arguments, span, environment)
                .map(|value| Value::boolean(!value.primary_value().is_truthy())),
            crate::Function::Constantly { value } => Ok(value.clone()),
            crate::Function::Primitive { name } => {
                self.apply_primitive(name, arguments, environment, span)
            }
            crate::Function::Generic {
                name,
                methods,
                method_combination,
                ..
            } => self.apply_generic(
                name,
                method_combination.clone(),
                methods,
                arguments,
                span,
                environment,
            ),
            crate::Function::SlotReader {
                class_name,
                slot_name,
            } => self.apply_slot_reader(class_name, slot_name, arguments, span, environment),
            crate::Function::SlotWriter {
                class_name,
                slot_name,
            } => self.apply_slot_writer(class_name, slot_name, arguments, span),
            crate::Function::SlotSetfWriter {
                class_name,
                slot_name,
            } => self.apply_slot_writer(class_name, slot_name, arguments, span),
            crate::Function::Method { definition } => {
                self.invoke_method(definition, arguments, None, span, environment)
            }
            crate::Function::ConditionReader {
                condition_name,
                slot_name,
            } => Self::apply_condition_reader(condition_name, slot_name, arguments, span),
            crate::Function::ConditionWriter {
                condition_name,
                slot_name,
            } => Self::apply_condition_writer(condition_name, slot_name, arguments, span),
            crate::Function::StructureConstructor {
                name,
                slots,
                structure_types,
                constructor_lambda_list,
                environment: definition_environment,
            } => self.apply_structure_constructor(&StructureConstructorContext {
                name,
                slots,
                structure_types,
                constructor_lambda_list: constructor_lambda_list.as_ref(),
                definition_environment,
                arguments,
                span,
            }),
            crate::Function::StructurePredicate { name } => {
                Self::apply_structure_predicate(name, arguments)
            }
            crate::Function::StructureAccessor {
                structure_name,
                slot_name: _,
                slot_index,
                ..
            } => Self::apply_structure_accessor(structure_name, *slot_index, arguments, span),
            crate::Function::StructureCopier { name } => {
                Self::apply_structure_copier(name, arguments, span)
            }
            crate::Function::Closure {
                parameters,
                required_escaped,
                optional,
                rest,
                rest_escaped,
                keywords,
                has_keyword_section,
                allow_other_keys,
                auxiliary,
                body,
                environment,
            } => self.apply_closure(&ClosureApplicationContext {
                parameters,
                required_escaped,
                optional,
                rest: rest.as_ref(),
                rest_escaped: *rest_escaped,
                keywords,
                has_keyword_section: *has_keyword_section,
                allow_other_keys: *allow_other_keys,
                auxiliary,
                body,
                environment,
                arguments,
                span,
            }),
            crate::Function::HashTableIterator { entries, index } => {
                if !arguments.is_empty() {
                    return Err(Self::arity("hash-table-iterator", "zero", arguments.len()));
                }
                let next = index.get();
                let Some((key, value)) = entries.get(next) else {
                    return Ok(Value::values(vec![Value::Nil, Value::Nil, Value::Nil]));
                };
                index.set(next.saturating_add(1));
                Ok(Value::values(vec![
                    Value::boolean(true),
                    key.clone(),
                    value.clone(),
                ]))
            }
            crate::Function::Macro { .. } | crate::Function::ModifyMacro { .. } => {
                Err(RuntimeError::NotCallable {
                    value: Value::Function(function.clone()).to_string(),
                    span: Some(span),
                })
            }
            crate::Function::Compiled {
                program,
                function,
                environment,
            } => crate::vm::run(self, program, *function, environment, arguments, span),
        }
    }
}
