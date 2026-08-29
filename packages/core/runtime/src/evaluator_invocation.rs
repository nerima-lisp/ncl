struct StructureBoaConstructorContext<'a> {
    name: &'a str,
    slots: &'a [StructureSlot],
    structure_types: &'a [String],
    lambda_list: &'a OrdinaryLambdaList,
    definition_environment: &'a Environment,
    arguments: &'a [Value],
    span: Span,
}

struct StructureConstructorContext<'a> {
    name: &'a str,
    slots: &'a [StructureSlot],
    structure_types: &'a [String],
    constructor_lambda_list: Option<&'a OrdinaryLambdaList>,
    definition_environment: &'a Environment,
    arguments: &'a [Value],
    span: Span,
}

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
}

struct StructureBoaBindingContext<'a, F, D>
where
    F: Fn(&str) -> Option<usize>,
    D: Fn(&str) -> Result<Value, RuntimeError>,
{
    lambda_list: &'a OrdinaryLambdaList,
    arguments: &'a [Value],
    required_count: usize,
    optional_supplied_count: usize,
    local: &'a Environment,
    slot_index: &'a F,
    evaluate_slot_default: &'a D,
    slot_values: &'a mut [Option<Value>],
}

struct StructureBoaKeywordContext<'a, F, D>
where
    F: Fn(&str) -> Option<usize>,
    D: Fn(&str) -> Result<Value, RuntimeError>,
{
    lambda_list: &'a OrdinaryLambdaList,
    arguments: &'a [Value],
    key_start: usize,
    span: Span,
    local: &'a Environment,
    slot_index: &'a F,
    evaluate_slot_default: &'a D,
    slot_values: &'a mut [Option<Value>],
}

impl Runtime {
    fn special_mapcar(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "mapcar",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let function = self.eval_in(&items[1], environment)?;
        let sequences = items[2..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_list_mapping("MAPCAR", &function, &sequences, environment, items[0].span)
    }

    fn special_map_into(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 3 {
            return Err(Self::arity(
                "map-into",
                "at least two",
                items.len().saturating_sub(1),
            ));
        }
        let destination_form = &items[1];
        let destination = self.eval_in(destination_form, environment)?;
        let function = self.eval_in(&items[2], environment)?;
        let sequences = items[3..]
            .iter()
            .map(|form| self.eval_in(form, environment))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.apply_sequence_map_into(
            &destination,
            &function,
            &sequences,
            environment,
            items[0].span,
        )?;
        self.set_map_into_destination(destination_form, result.clone(), environment)?;
        Ok(result)
    }

    fn make_instance(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.is_empty() {
            return Err(Self::arity(
                "make-instance",
                "at least one",
                arguments.len(),
            ));
        }
        if !(arguments.len() - 1).is_multiple_of(2) {
            return Err(Self::invalid("make-instance initargs require pairs", span));
        }
        let class_name = Self::name_designator_from_value(&arguments[0], span)?;
        let class = environment
            .lookup_class(&class_name)
            .ok_or_else(|| Self::invalid("unknown class", span))?;

        let mut initargs = Vec::with_capacity(arguments.len().saturating_sub(1));
        for pair in arguments[1..].as_chunks::<2>().0 {
            let initarg = Self::name_designator_from_value(&pair[0], span)?;
            initargs.push((initarg, pair[1].clone()));
        }
        for (initarg, init_form) in &class.default_initargs {
            if initargs.iter().any(|(name, _)| name == initarg) {
                continue;
            }
            initargs.push((initarg.clone(), self.eval_in(init_form, environment)?));
        }
        for (initarg, _) in &initargs {
            if !class
                .slots
                .iter()
                .any(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
            {
                return Err(Self::invalid("unknown make-instance initarg", span));
            }
        }

        let mut slots = Vec::with_capacity(class.slots.len());
        for slot in &class.slots {
            let initarg_value = slot.initarg.as_ref().and_then(|initarg| {
                initargs
                    .iter()
                    .rev()
                    .find(|(name, _)| name == initarg)
                    .map(|(_, value)| value.clone())
            });
            let value = if let Some(initarg_value) = initarg_value {
                initarg_value
            } else if let Some(class_value) = &slot.class_value {
                let current = class_value.borrow().clone();
                if matches!(current, Value::Unbound) {
                    let value = slot
                        .init_form
                        .as_ref()
                        .map(|form| self.eval_in(form, environment))
                        .transpose()?
                        .unwrap_or(Value::Unbound);
                    *class_value.borrow_mut() = value.clone();
                    value
                } else {
                    current
                }
            } else {
                slot.init_form
                    .as_ref()
                    .map(|form| self.eval_in(form, environment))
                    .transpose()?
                    .unwrap_or(Value::Unbound)
            };
            slots.push((slot.name.clone(), value));
        }
        let instance = Value::instance(class.clone(), slots);
        for (initarg, value) in initargs {
            let Some(index) = class
                .slots
                .iter()
                .position(|slot| slot.initarg.as_deref() == Some(initarg.as_str()))
            else {
                return Err(Self::invalid("unknown make-instance initarg", span));
            };
            if !instance.set_instance_slot(&class.name, &class.slots[index].name, value) {
                return Err(Self::invalid("unknown make-instance initarg", span));
            }
        }
        Ok(instance)
    }

    fn compile_function(
        &self,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if !(1..=2).contains(&arguments.len()) {
            return Err(Self::arity("compile", "one or two", arguments.len()));
        }

        let name = match &arguments[0] {
            Value::Nil | Value::Boolean(false) => None,
            value => {
                let (name, exact) = value
                    .symbol_reference()
                    .ok_or_else(|| Self::invalid("compile name must be a symbol or NIL", span))?;
                Some((name.to_owned(), exact))
            }
        };

        let function = match arguments.get(1) {
            None | Some(Value::Nil | Value::Boolean(false)) => {
                let Some((name, exact)) = name.as_ref() else {
                    return Err(Self::invalid(
                        "compile needs a function definition when the name is NIL",
                        span,
                    ));
                };
                let function = if *exact {
                    self.lookup_function_exact_in(name, environment)
                } else {
                    self.lookup_function_in(name, environment)
                };
                match function {
                    Some(value @ Value::Function(_)) => value,
                    Some(value) => {
                        return Err(RuntimeError::NotCallable {
                            value: value.to_string(),
                            span: Some(span),
                        });
                    }
                    None => {
                        return Err(RuntimeError::UnboundVariable {
                            name: name.clone(),
                            span: Some(span),
                        });
                    }
                }
            }
            Some(definition) => {
                let form = Self::form_from_value(definition, span)?;
                let expanded = self.prepare_compiled_form(&form, environment)?;
                let program = Rc::new(Compiler::compile_form(&expanded)?);
                crate::vm::run_entry(self, &program, 0, environment, expanded.span)?.primary_value()
            }
        };

        if !matches!(function, Value::Function(_)) {
            return Err(RuntimeError::Type {
                expected: "FUNCTION".to_owned(),
                actual: function.type_name().to_owned(),
                span: Some(span),
            });
        }

        if let Some((name, exact)) = name {
            if exact {
                environment.define_function_exact(name, function.clone());
            } else {
                environment.define_function(name, function.clone());
            }
        }

        Ok(Value::values(vec![function, Value::Nil, Value::Nil]))
    }

    fn load_file(&self, arguments: &[Value], span: Span) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("load", "one", arguments.len()));
        }
        let path = match &arguments[0] {
            Value::String(path) => path.to_string(),
            value => {
                return Err(RuntimeError::Type {
                    expected: "PATHNAME-DESIGNATOR".to_owned(),
                    actual: value.type_name().to_owned(),
                    span: Some(span),
                });
            }
        };
        let source = fs::read_to_string(&path)
            .map_err(|error| RuntimeError::Io(format!("cannot load {path}: {error}")))?;
        self.eval_source(&source)?;
        Ok(Value::boolean(true))
    }

    fn method_score(method: &MethodDefinition, arguments: &[Value]) -> Option<usize> {
        let required_count = method.specializers.len();
        if arguments.len() < required_count {
            return None;
        }
        if let Value::Function(function) = &method.function
            && let crate::Function::Closure {
                parameters,
                optional,
                rest,
                has_keyword_section,
                ..
            } = function.as_ref()
            && (parameters.len() != required_count
                || (!*has_keyword_section
                    && rest.is_none()
                    && arguments.len() > required_count + optional.len()))
        {
            return None;
        }
        let mut score = 0usize;
        for (specializer, argument) in method
            .specializers
            .iter()
            .zip(arguments.iter().take(required_count))
        {
            if specializer == "T" || specializer == "OBJECT" {
                score = score.saturating_add(1_000_000);
                continue;
            }
            let class = argument.instance_class_definition()?;
            let position = class
                .precedence
                .iter()
                .position(|name| name == specializer)?;
            score = score.saturating_add(position);
        }
        Some(score)
    }

    fn invoke_method(
        &self,
        method: &MethodDefinition,
        arguments: &[Value],
        next: Option<MethodContinuation>,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.method_context.borrow_mut().push(MethodContext {
            arguments: arguments.to_vec(),
            next,
        });
        let result = self.apply_in(&method.function, arguments, span, environment);
        self.method_context.borrow_mut().pop();
        result
    }

    fn invoke_core(
        &self,
        before: &[MethodDefinition],
        primary: &[MethodDefinition],
        after: &[MethodDefinition],
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for method in before {
            self.invoke_method(method, arguments, None, span, environment)?;
        }
        let Some(method) = primary.first() else {
            return Err(Self::invalid("no primary method is applicable", span));
        };
        let next = (primary.len() > 1).then(|| MethodContinuation::Chain {
            methods: primary.to_vec(),
            index: 1,
            fallback: None,
        });
        let result = self.invoke_method(method, arguments, next, span, environment)?;
        for method in after {
            self.invoke_method(method, arguments, None, span, environment)?;
        }
        Ok(result)
    }

    fn invoke_continuation(
        &self,
        continuation: MethodContinuation,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match continuation {
            MethodContinuation::Chain {
                methods,
                index,
                fallback,
            } => {
                if index < methods.len() {
                    let method = methods[index].clone();
                    let next = if index + 1 < methods.len() || fallback.is_some() {
                        Some(MethodContinuation::Chain {
                            methods,
                            index: index + 1,
                            fallback,
                        })
                    } else {
                        None
                    };
                    self.invoke_method(&method, arguments, next, span, environment)
                } else if let Some(fallback) = fallback {
                    self.invoke_continuation(*fallback, arguments, span, environment)
                } else {
                    Err(Self::invalid("no next method is applicable", span))
                }
            }
            MethodContinuation::Core {
                before,
                primary,
                after,
            } => self.invoke_core(&before, &primary, &after, arguments, span, environment),
        }
    }

    fn apply_generic(
        &self,
        name: &str,
        methods: &RefCell<Vec<MethodDefinition>>,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut applicable = methods
            .borrow()
            .iter()
            .filter_map(|method| {
                Self::method_score(method, arguments).map(|score| (score, method.clone()))
            })
            .collect::<Vec<_>>();
        if applicable.is_empty() {
            return Err(Self::invalid(
                &format!("no applicable method for {name}"),
                span,
            ));
        }
        applicable.sort_by_key(|(score, _)| *score);

        let mut around = Vec::new();
        let mut before = Vec::new();
        let mut primary = Vec::new();
        let mut after = Vec::new();
        for (_, method) in applicable {
            match method.qualifiers.first().map(String::as_str) {
                Some("AROUND") => around.push(method),
                Some("BEFORE") => before.push(method),
                Some("AFTER") => after.push(method),
                _ => primary.push(method),
            }
        }
        after.reverse();
        let core = MethodContinuation::Core {
            before,
            primary,
            after,
        };
        if around.is_empty() {
            self.invoke_continuation(core, arguments, span, environment)
        } else {
            let first = around[0].clone();
            let next = MethodContinuation::Chain {
                methods: around,
                index: 1,
                fallback: Some(Box::new(core)),
            };
            self.invoke_method(&first, arguments, Some(next), span, environment)
        }
    }

    pub(crate) fn apply_in(
        &self,
        function: &Value,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let function = self.resolve_function_designator(function, span, environment)?;
        match function.as_ref() {
            crate::Function::Builtin { function, .. } => function(arguments),
            crate::Function::Primitive { name } => {
                self.apply_primitive(name, arguments, environment, span)
            }
            crate::Function::Generic { name, methods } => {
                self.apply_generic(name, methods, arguments, span, environment)
            }
            crate::Function::SlotReader {
                class_name,
                slot_name,
            } => Self::apply_slot_reader(class_name, slot_name, arguments, span),
            crate::Function::SlotWriter {
                class_name,
                slot_name,
            } => Self::apply_slot_writer(class_name, slot_name, arguments, span),
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

    fn apply_slot_reader(
        class_name: &str,
        slot_name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("slot reader", "one", arguments.len()));
        }
        if !arguments[0].instance_is_type(class_name) {
            return Err(RuntimeError::Type {
                expected: class_name.to_string(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        }
        let value = arguments[0]
            .instance_slot(slot_name)
            .ok_or_else(|| Self::invalid("slot is not defined for this class", span))?;
        if matches!(value, Value::Unbound) {
            return Err(Self::invalid("slot is unbound", span));
        }
        Ok(value)
    }

    fn apply_slot_writer(
        class_name: &str,
        slot_name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("slot writer", "two", arguments.len()));
        }
        let value = arguments[0].clone();
        let object = &arguments[1];
        if !object.instance_is_type(class_name) {
            return Err(RuntimeError::Type {
                expected: class_name.to_string(),
                actual: object.type_name().to_string(),
                span: Some(span),
            });
        }
        if object.set_instance_slot(class_name, slot_name, value.clone()) {
            Ok(value)
        } else {
            Err(Self::invalid("slot is not defined for this class", span))
        }
    }

    fn apply_condition_reader(
        condition_name: &str,
        slot_name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("condition reader", "one", arguments.len()));
        }
        arguments[0]
            .condition_slot(condition_name, slot_name)
            .ok_or_else(|| Self::invalid("condition slot is not defined", span))
    }

    fn apply_condition_writer(
        condition_name: &str,
        slot_name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 2 {
            return Err(Self::arity("condition writer", "two", arguments.len()));
        }
        let value = arguments[0].clone();
        let object = &arguments[1];
        if object.set_condition_slot(condition_name, slot_name, value.clone()) {
            Ok(value)
        } else {
            Err(Self::invalid("condition slot is not defined", span))
        }
    }

    fn apply_structure_constructor(
        &self,
        context: &StructureConstructorContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let StructureConstructorContext {
            name,
            slots,
            structure_types,
            constructor_lambda_list,
            definition_environment,
            arguments,
            span,
        } = *context;
        if let Some(lambda_list) = constructor_lambda_list {
            return self.apply_structure_boa_constructor(&StructureBoaConstructorContext {
                name,
                slots,
                structure_types,
                lambda_list,
                definition_environment,
                arguments,
                span,
            });
        }
        if !arguments.len().is_multiple_of(2) {
            return Err(Self::arity(
                "structure constructor",
                "an even number of",
                arguments.len(),
            ));
        }
        let mut supplied = vec![None; slots.len()];
        for pair in arguments.as_chunks::<2>().0 {
            let keyword_name = match &pair[0] {
                Value::Keyword(keyword) | Value::KeywordExact(keyword) => normalize_name(keyword),
                _ => {
                    return Err(Self::invalid(
                        "structure constructor keyword name must be a keyword",
                        span,
                    ));
                }
            };
            let Some(index) = slots.iter().position(|slot| slot.name == keyword_name) else {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unknown structure keyword :{keyword_name}"),
                    span: Some(span),
                });
            };
            supplied[index] = Some(pair[1].clone());
        }
        let mut values = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            let value = match supplied[index].clone() {
                Some(value) => value,
                None => slot
                    .init_form
                    .as_ref()
                    .map(|form| self.eval_in(form, definition_environment))
                    .transpose()?
                    .unwrap_or(Value::Nil),
            };
            values.push((slot.name.clone(), value));
        }
        Ok(Value::structure_with_types(
            name,
            values,
            structure_types.to_vec(),
        ))
    }

    fn apply_structure_predicate(name: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("structure predicate", "one", arguments.len()));
        }
        Ok(Value::boolean(arguments[0].structure_is_type(name)))
    }

    fn apply_structure_accessor(
        structure_name: &str,
        slot_index: usize,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("structure accessor", "one", arguments.len()));
        }
        if !arguments[0].structure_is_type(structure_name) {
            return Err(RuntimeError::Type {
                expected: structure_name.to_string(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        }
        arguments[0]
            .structure_slot(slot_index)
            .ok_or_else(|| Self::invalid("structure slot is out of range", span))
    }

    fn apply_structure_copier(
        name: &str,
        arguments: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if arguments.len() != 1 {
            return Err(Self::arity("structure copier", "one", arguments.len()));
        }
        if !arguments[0].structure_is_type(name) {
            return Err(RuntimeError::Type {
                expected: name.to_string(),
                actual: arguments[0].type_name().to_string(),
                span: Some(span),
            });
        }
        arguments[0]
            .copy_structure()
            .ok_or_else(|| Self::invalid("structure copy failed", span))
    }

    fn apply_structure_boa_constructor(
        &self,
        context: &StructureBoaConstructorContext<'_>,
    ) -> Result<Value, RuntimeError> {
        let StructureBoaConstructorContext {
            name,
            slots,
            structure_types,
            lambda_list,
            definition_environment,
            arguments,
            span,
        } = *context;
        let (required_count, optional_supplied_count, key_start) =
            Self::structure_boa_argument_counts(lambda_list, arguments)?;

        let local = definition_environment.child();
        let _dynamic_guard = self.dynamic_guard();
        let mut slot_values = vec![None; slots.len()];
        let slot_index =
            |parameter_name: &str| slots.iter().position(|slot| slot.name == parameter_name);
        let evaluate_slot_default = |parameter_name: &str| -> Result<Value, RuntimeError> {
            slots
                .iter()
                .find(|slot| slot.name == parameter_name)
                .and_then(|slot| slot.init_form.as_ref())
                .map(|form| self.eval_in(form, definition_environment))
                .transpose()
                .map(|value| value.unwrap_or(Value::Nil))
        };

        let binding_context = StructureBoaBindingContext {
            lambda_list,
            arguments,
            required_count,
            optional_supplied_count,
            local: &local,
            slot_index: &slot_index,
            evaluate_slot_default: &evaluate_slot_default,
            slot_values: &mut slot_values,
        };
        self.bind_structure_boa_required_optional(binding_context)?;

        if let Some(rest) = &lambda_list.rest {
            let value = Value::list(arguments[key_start..].to_vec());
            if lambda_list.rest_escaped {
                self.define_exact_in(rest, value.clone(), &local);
            } else {
                self.define_in(rest, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(rest) {
                slot_values[slot_index] = Some(value);
            }
        }

        if lambda_list.has_keyword_section {
            let keyword_context = StructureBoaKeywordContext {
                lambda_list,
                arguments,
                key_start,
                span,
                local: &local,
                slot_index: &slot_index,
                evaluate_slot_default: &evaluate_slot_default,
                slot_values: &mut slot_values,
            };
            self.bind_structure_boa_keywords(keyword_context)?;
        }

        for specification in &lambda_list.auxiliary {
            let value = self.eval_in(&specification.init_form, &local)?;
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), &local);
            } else {
                self.define_in(&specification.name, value.clone(), &local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
        }

        let mut values = Vec::with_capacity(slots.len());
        for (index, slot) in slots.iter().enumerate() {
            let value = match slot_values[index].take() {
                Some(value) => value,
                None => evaluate_slot_default(&slot.name)?,
            };
            values.push((slot.name.clone(), value));
        }
        Ok(Value::structure_with_types(
            name,
            values,
            structure_types.to_vec(),
        ))
    }

    fn bind_structure_boa_keywords<F, D>(
        &self,
        context: StructureBoaKeywordContext<'_, F, D>,
    ) -> Result<(), RuntimeError>
    where
        F: Fn(&str) -> Option<usize>,
        D: Fn(&str) -> Result<Value, RuntimeError>,
    {
        let StructureBoaKeywordContext {
            lambda_list,
            arguments,
            key_start,
            span,
            local,
            slot_index,
            evaluate_slot_default,
            slot_values,
        } = context;
        let keyword_arguments = &arguments[key_start..];
        if !keyword_arguments.len().is_multiple_of(2) {
            return Err(Self::invalid(
                "keyword arguments must be supplied in pairs",
                span,
            ));
        }
        let mut supplied_keywords = HashMap::new();
        let mut accepts_unknown_keywords = lambda_list.allow_other_keys;
        for pair in keyword_arguments.as_chunks::<2>().0 {
            let (Value::Keyword(keyword) | Value::KeywordExact(keyword)) = &pair[0] else {
                return Err(Self::invalid(
                    "keyword argument name must be a keyword",
                    span,
                ));
            };
            let keyword_name = keyword.to_string();
            if keyword_name == "ALLOW-OTHER-KEYS" && pair[1].is_truthy() {
                accepts_unknown_keywords = true;
            }
            supplied_keywords.insert(keyword_name, pair[1].clone());
        }
        if !accepts_unknown_keywords {
            for keyword_name in supplied_keywords.keys() {
                if keyword_name != "ALLOW-OTHER-KEYS"
                    && !lambda_list
                        .keywords
                        .iter()
                        .any(|specification| specification.keyword_name == *keyword_name)
                {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unknown keyword :{keyword_name}"),
                        span: Some(span),
                    });
                }
            }
        }
        for specification in &lambda_list.keywords {
            let supplied = supplied_keywords.get(&specification.keyword_name);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None if specification.init_form_supplied => {
                    self.eval_in(&specification.init_form, local)?
                }
                None => evaluate_slot_default(&specification.name)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), local);
            } else {
                self.define_in(&specification.name, value.clone(), local);
            }
            if let Some(index) = slot_index(&specification.name) {
                slot_values[index] = Some(value);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, local);
                } else {
                    self.define_in(supplied_p, supplied_value, local);
                }
            }
        }
        Ok(())
    }

    fn bind_structure_boa_required_optional<F, D>(
        &self,
        context: StructureBoaBindingContext<'_, F, D>,
    ) -> Result<(), RuntimeError>
    where
        F: Fn(&str) -> Option<usize>,
        D: Fn(&str) -> Result<Value, RuntimeError>,
    {
        let StructureBoaBindingContext {
            lambda_list,
            arguments,
            required_count,
            optional_supplied_count,
            local,
            slot_index,
            evaluate_slot_default,
            slot_values,
        } = context;
        for (index, (parameter, argument)) in lambda_list
            .required
            .iter()
            .zip(arguments.iter())
            .enumerate()
        {
            if lambda_list
                .required_escaped
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                self.define_exact_in(parameter, argument.clone(), local);
            } else {
                self.define_in(parameter, argument.clone(), local);
            }
            if let Some(slot_index) = slot_index(parameter) {
                slot_values[slot_index] = Some(argument.clone());
            }
        }
        for (index, specification) in lambda_list.optional.iter().enumerate() {
            let supplied =
                (index < optional_supplied_count).then(|| &arguments[required_count + index]);
            let value = match supplied {
                Some(argument) => argument.clone(),
                None if specification.init_form_supplied => {
                    self.eval_in(&specification.init_form, local)?
                }
                None => evaluate_slot_default(&specification.name)?,
            };
            if specification.name_escaped {
                self.define_exact_in(&specification.name, value.clone(), local);
            } else {
                self.define_in(&specification.name, value.clone(), local);
            }
            if let Some(slot_index) = slot_index(&specification.name) {
                slot_values[slot_index] = Some(value);
            }
            if let Some(supplied_p) = &specification.supplied_p {
                let supplied_value = Value::boolean(supplied.is_some());
                if specification.supplied_p_escaped.unwrap_or(false) {
                    self.define_exact_in(supplied_p, supplied_value, local);
                } else {
                    self.define_in(supplied_p, supplied_value, local);
                }
            }
        }
        Ok(())
    }
}
mod closure;
mod structure_boa;
