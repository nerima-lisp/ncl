use std::cell::RefCell;
use std::rc::Rc;

use ncl_compiler::{FunctionId, Program};
use ncl_syntax::{Form, LambdaListAuxiliaryParameter, LambdaListOptionalParameter};

use super::{Builtin, ClosureOptions, Environment, Function, MacroLambdaList, Value};

impl Value {
    pub(crate) fn complement(function: Self) -> Self {
        Self::Function(Rc::new(Function::Complement { function }))
    }

    /// Creates a callable value backed by a runtime builtin.
    pub fn builtin(name: &'static str, function: Builtin) -> Self {
        Self::Function(Rc::new(Function::Builtin { name, function }))
    }

    pub(crate) fn primitive(name: &'static str) -> Self {
        Self::Function(Rc::new(Function::Primitive { name }))
    }

    pub(crate) fn generic(name: impl Into<String>) -> Self {
        Self::Function(Rc::new(Function::Generic {
            name: name.into(),
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

    /// Creates a closure with required parameters and a lexical environment.
    #[must_use]
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
        Self::closure_with_keywords(
            ClosureOptions {
                parameters,
                required_escaped,
                optional,
                rest,
                rest_escaped: false,
                keywords: Vec::new(),
                has_keyword_section: false,
                allow_other_keys: false,
                auxiliary,
            },
            body,
            environment,
        )
    }

    pub(crate) fn closure_with_keywords(
        options: ClosureOptions,
        body: Vec<Form>,
        environment: Environment,
    ) -> Self {
        Self::Function(Rc::new(Function::Closure {
            parameters: options.parameters,
            required_escaped: options.required_escaped,
            optional: options.optional,
            rest: options.rest,
            rest_escaped: options.rest_escaped,
            keywords: options.keywords,
            has_keyword_section: options.has_keyword_section,
            allow_other_keys: options.allow_other_keys,
            auxiliary: options.auxiliary,
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
}
