impl Value {
    pub fn boolean(value: bool) -> Self {
        if value {
            Self::Boolean(true)
        } else {
            Self::Nil
        }
    }

    pub fn string(value: impl Into<Rc<str>>) -> Self {
        Self::String(value.into())
    }

    pub(crate) fn rational(numerator: i128, denominator: i128) -> Result<Self, RuntimeError> {
        let rational = Rational::new(numerator, denominator)?;
        if rational.denominator() == 1 {
            Ok(Self::Integer(rational.numerator()))
        } else {
            Ok(Self::Rational(rational))
        }
    }

    pub(crate) fn complex(real: Self, imag: Self) -> Self {
        Self::Complex {
            real: Rc::new(real),
            imag: Rc::new(imag),
        }
    }

    pub(crate) fn string_input_stream(source: &str, start: usize, end: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::input(source, start, end))))
    }

    pub(crate) fn string_output_stream() -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::output())))
    }

    pub(crate) fn file_input_stream(source: String) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_input(source))))
    }

    pub(crate) fn file_output_stream(path: PathBuf, initial: String) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_output(path, initial))))
    }

    pub(crate) fn file_io_stream(path: PathBuf, source: String, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_io(path, source, append))))
    }

    pub fn package(value: impl AsRef<str>) -> Self {
        Self::Package(Rc::from(value.as_ref()))
    }

    pub(crate) fn environment(value: Environment) -> Self {
        Self::Environment(value)
    }

    pub fn symbol(value: impl AsRef<str>) -> Self {
        Self::Symbol(Rc::from(value.as_ref().to_ascii_uppercase().as_str()))
    }

    pub fn symbol_exact(value: impl AsRef<str>) -> Self {
        Self::SymbolExact(Rc::from(value.as_ref()))
    }

    pub fn uninterned_symbol(value: impl AsRef<str>) -> Self {
        Self::UninternedSymbol(Rc::from(value.as_ref()))
    }

    pub fn keyword(value: impl AsRef<str>) -> Self {
        let value = value.as_ref().trim_start_matches(':').to_ascii_uppercase();
        Self::Keyword(Rc::from(value))
    }

    pub fn keyword_exact(value: impl AsRef<str>) -> Self {
        Self::KeywordExact(Rc::from(value.as_ref().trim_start_matches(':')))
    }

    pub fn list(values: Vec<Self>) -> Self {
        if values.is_empty() {
            Self::Nil
        } else {
            Self::List(Rc::new(values))
        }
    }

    pub fn dotted_list(items: Vec<Self>, tail: Self) -> Self {
        Self::DottedList {
            items: Rc::new(items),
            tail: Rc::new(tail),
        }
    }

    pub fn vector(values: Vec<Self>) -> Self {
        Self::vector_with_fill_pointer_element_type_adjustable_and_displacement(
            values,
            None,
            Self::symbol("T"),
            false,
            None,
            0,
        )
    }

    pub fn vector_with_fill_pointer(values: Vec<Self>, fill_pointer: usize) -> Self {
        Self::vector_with_fill_pointer_element_type_adjustable_and_displacement(
            values,
            Some(fill_pointer),
            Self::symbol("T"),
            false,
            None,
            0,
        )
    }

    pub fn array(dimensions: Vec<usize>, elements: Vec<Self>) -> Self {
        Self::array_with_element_type_adjustable_and_displacement(
            dimensions,
            elements,
            Self::symbol("T"),
            false,
            None,
            0,
        )
    }

    pub fn vector_with_fill_pointer_and_element_type(
        values: Vec<Self>,
        fill_pointer: Option<usize>,
        element_type: Self,
    ) -> Self {
        Self::vector_with_fill_pointer_element_type_adjustable_and_displacement(
            values,
            fill_pointer,
            element_type,
            false,
            None,
            0,
        )
    }

    pub fn vector_with_fill_pointer_element_type_and_adjustable(
        values: Vec<Self>,
        fill_pointer: Option<usize>,
        element_type: Self,
        adjustable: bool,
    ) -> Self {
        Self::vector_with_fill_pointer_element_type_adjustable_and_displacement(
            values,
            fill_pointer,
            element_type,
            adjustable,
            None,
            0,
        )
    }

    pub fn vector_with_fill_pointer_element_type_adjustable_and_displacement(
        values: Vec<Self>,
        fill_pointer: Option<usize>,
        element_type: Self,
        adjustable: bool,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        let length = values.len();
        Self::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
            Rc::new(RefCell::new(values)),
            length,
            fill_pointer,
            element_type,
            adjustable,
            displaced_to,
            displaced_index_offset,
        )
    }

    pub(crate) fn vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
        elements: Rc<RefCell<Vec<Self>>>,
        length: usize,
        fill_pointer: Option<usize>,
        element_type: Self,
        adjustable: bool,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        Self::Vector {
            length,
            elements,
            fill_pointer,
            element_type: Rc::new(element_type),
            adjustable,
            displaced_to: displaced_to.map(Rc::new),
            displaced_index_offset,
        }
    }

    pub fn array_with_element_type(
        dimensions: Vec<usize>,
        elements: Vec<Self>,
        element_type: Self,
    ) -> Self {
        Self::array_with_element_type_adjustable_and_displacement(
            dimensions,
            elements,
            element_type,
            false,
            None,
            0,
        )
    }

    pub fn array_with_element_type_and_adjustable(
        dimensions: Vec<usize>,
        elements: Vec<Self>,
        element_type: Self,
        adjustable: bool,
    ) -> Self {
        Self::array_with_element_type_adjustable_and_displacement(
            dimensions,
            elements,
            element_type,
            adjustable,
            None,
            0,
        )
    }

    pub fn array_with_element_type_adjustable_and_displacement(
        dimensions: Vec<usize>,
        elements: Vec<Self>,
        element_type: Self,
        adjustable: bool,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        Self::array_with_storage_element_type_adjustable_and_displacement(
            dimensions,
            Rc::new(RefCell::new(elements)),
            element_type,
            adjustable,
            displaced_to,
            displaced_index_offset,
        )
    }

    pub(crate) fn array_with_storage_element_type_adjustable_and_displacement(
        dimensions: Vec<usize>,
        elements: Rc<RefCell<Vec<Self>>>,
        element_type: Self,
        adjustable: bool,
        displaced_to: Option<Self>,
        displaced_index_offset: usize,
    ) -> Self {
        Self::Array {
            dimensions: Rc::new(dimensions),
            elements,
            element_type: Rc::new(element_type),
            adjustable,
            displaced_to: displaced_to.map(Rc::new),
            displaced_index_offset,
        }
    }

    pub(crate) fn hash_table(test: impl AsRef<str>) -> Self {
        Self::HashTable {
            test: Rc::from(test.as_ref()),
            entries: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub(crate) fn values(values: Vec<Self>) -> Self {
        Self::Values(Rc::new(values))
    }

    pub(crate) fn condition(error: &RuntimeError) -> Self {
        let (actual_type, type_names, message, format_control, format_arguments) = match error {
            RuntimeError::Signaled {
                condition,
                condition_types,
                message,
                format_control,
                format_arguments,
                ..
            } => (
                error.condition_type_name(),
                if condition_types.is_empty() {
                    vec![condition.clone()]
                } else {
                    condition_types.to_vec()
                },
                message.clone(),
                format_control.clone(),
                format_arguments
                    .iter()
                    .cloned()
                    .map(ReturnValue::into_value)
                    .collect(),
            ),
            _ => (
                error.condition_type_name(),
                vec![error.condition_type_name()],
                error.to_string(),
                None,
                Vec::new(),
            ),
        };
        Self::condition_from_parts_with_types(
            actual_type,
            type_names,
            Vec::new(),
            message,
            format_control,
            format_arguments,
        )
    }

    pub(crate) fn condition_from_parts(
        actual_type: String,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Value>,
    ) -> Self {
        Self::condition_from_parts_with_types(
            actual_type.clone(),
            vec![actual_type],
            Vec::new(),
            message,
            format_control,
            format_arguments,
        )
    }

    pub(crate) fn condition_from_definition(
        actual_type: String,
        type_names: Vec<String>,
        slots: Vec<(String, Value)>,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Value>,
    ) -> Self {
        Self::condition_from_parts_with_types(
            actual_type,
            type_names,
            slots,
            message,
            format_control,
            format_arguments,
        )
    }

    fn condition_from_parts_with_types(
        actual_type: String,
        type_names: Vec<String>,
        slots: Vec<(String, Value)>,
        message: String,
        format_control: Option<String>,
        format_arguments: Vec<Value>,
    ) -> Self {
        Self::Condition(Rc::new(ConditionData {
            actual_type,
            type_names: Rc::new(type_names),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(name, value)| (Rc::from(name.as_str()), value))
                    .collect(),
            )),
            message: Rc::from(message.as_str()),
            format_control: format_control.map(|value| Rc::from(value.as_str())),
            format_arguments,
        }))
    }

    pub(crate) fn restart(name: impl AsRef<str>) -> Self {
        Self::Restart(Rc::new(RestartData {
            name: Rc::from(name.as_ref()),
        }))
    }

    pub fn builtin(name: &'static str, function: Builtin) -> Self {
        Self::Function(Rc::new(Function::Builtin { name, function }))
    }

    pub(crate) fn primitive(name: &'static str) -> Self {
        Self::Function(Rc::new(Function::Primitive { name }))
    }

    pub(crate) fn generic(name: impl Into<String>, lambda_list: OrdinaryLambdaList) -> Self {
        Self::Function(Rc::new(Function::Generic {
            name: name.into(),
            lambda_list,
            methods: Rc::new(RefCell::new(Vec::new())),
        }))
    }

    pub(crate) fn slot_reader(class_name: impl Into<String>, slot_name: impl Into<String>) -> Self {
        Self::Function(Rc::new(Function::SlotReader {
            class_name: class_name.into(),
            slot_name: slot_name.into(),
        }))
    }

    pub(crate) fn slot_writer(class_name: impl Into<String>, slot_name: impl Into<String>) -> Self {
        Self::Function(Rc::new(Function::SlotWriter {
            class_name: class_name.into(),
            slot_name: slot_name.into(),
        }))
    }

    pub(crate) fn condition_reader(
        condition_name: impl Into<String>,
        slot_name: impl Into<String>,
    ) -> Self {
        Self::Function(Rc::new(Function::ConditionReader {
            condition_name: condition_name.into(),
            slot_name: slot_name.into(),
        }))
    }

    pub(crate) fn condition_writer(
        condition_name: impl Into<String>,
        slot_name: impl Into<String>,
    ) -> Self {
        Self::Function(Rc::new(Function::ConditionWriter {
            condition_name: condition_name.into(),
            slot_name: slot_name.into(),
        }))
    }

    pub fn closure(parameters: Vec<String>, body: Vec<Form>, environment: Environment) -> Self {
        Self::closure_with_optional(parameters, Vec::new(), None, body, environment)
    }

    pub(crate) fn closure_with_optional(
        parameters: Vec<String>,
        optional: Vec<LambdaListOptionalParameter>,
        rest: Option<String>,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::closure_with_auxiliary(parameters, optional, rest, Vec::new(), body, environment)
    }

    pub(crate) fn closure_with_auxiliary(
        parameters: Vec<String>,
        optional: Vec<LambdaListOptionalParameter>,
        rest: Option<String>,
        auxiliary: Vec<LambdaListAuxiliaryParameter>,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        let required_escaped = vec![false; parameters.len()];
        Self::closure_with_keywords(ClosureData {
            parameters,
            required_escaped,
            optional,
            rest,
            rest_escaped: false,
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            auxiliary,
            body,
            environment,
        })
    }

    pub(crate) fn closure_with_keywords(data: ClosureData) -> Self {
        let ClosureData {
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
        } = data;
        Self::Function(Rc::new(Function::Closure {
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
        }))
    }

    pub(crate) fn macro_function(
        lambda_list: MacroLambdaList,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::Macro {
            lambda_list,
            body,
            environment,
        }))
    }

    pub(crate) fn modify_macro_function(
        lambda_list: MacroLambdaList,
        function: Form,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::ModifyMacro {
            lambda_list,
            function,
            environment,
        }))
    }

    pub(crate) fn long_defsetf_function(
        lambda_list: MacroLambdaList,
        store_variable: String,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::LongDefsetf {
            lambda_list,
            store_variable,
            body,
            environment,
        }))
    }

    pub(crate) fn compiled(
        program: Rc<Program>,
        function: FunctionId,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::Compiled {
            program,
            function,
            environment,
        }))
    }

    pub(crate) fn structure_with_types(
        name: impl AsRef<str>,
        slots: Vec<(String, Value)>,
        mut type_names: Vec<String>,
    ) -> Self {
        let name = name.as_ref().to_string();
        if !type_names
            .iter()
            .any(|type_name| type_name.eq_ignore_ascii_case(&name))
        {
            type_names.insert(0, name.clone());
        }
        Self::Structure {
            name: Rc::from(name),
            types: Rc::new(type_names.into_iter().map(Rc::<str>::from).collect()),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(slot_name, value)| (Rc::from(slot_name), value))
                    .collect(),
            )),
        }
    }

    pub(crate) fn class_object(definition: Rc<ClassDefinition>) -> Self {
        Self::Class(definition)
    }

    pub(crate) fn instance(definition: Rc<ClassDefinition>, slots: Vec<(String, Value)>) -> Self {
        Self::Instance(Instance {
            class: Rc::new(RefCell::new(definition)),
            slots: Rc::new(RefCell::new(
                slots
                    .into_iter()
                    .map(|(slot_name, value)| (Rc::from(slot_name), value))
                    .collect(),
            )),
        })
    }

}
