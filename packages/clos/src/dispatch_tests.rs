#[cfg(test)]
mod included_tests {
    use super::super::COMMON_LISP;
    use ncl_object::{
        FunctionArguments, FunctionCaller, FunctionObject, MultipleValues, Package, Runtime, Scope,
        ThreadContext, Word, make_code_object, make_simple_fun, push_heap_root,
    };

    struct RecordingCaller {
        calls: Vec<(Word, Vec<Word>)>,
    }

    impl RecordingCaller {
        fn new() -> Self {
            Self { calls: Vec::new() }
        }
    }

    impl FunctionCaller for RecordingCaller {
        fn call_function(
            &mut self,
            _ctx: &mut ThreadContext,
            _runtime: &Runtime,
            designator: ncl_object::typed::FunctionDesignator,
            args: FunctionArguments<'_>,
            values: &mut MultipleValues,
        ) -> Result<Word, ncl_object::ObjectError> {
            let function = match designator {
                ncl_object::typed::FunctionDesignator::Function(function) => function.as_word(),
                ncl_object::typed::FunctionDesignator::Symbol(_) => {
                    return Err(ncl_object::ObjectError::UndefinedFunction);
                }
            };
            self.calls.push((function, args.as_slice().to_vec()));
            let result = Word::fixnum(self.calls.len() as i64);
            values.set(&[result, Word::fixnum(900 + self.calls.len() as i64)]);
            Ok(result)
        }
    }

    fn simple_function(context: &mut ThreadContext, runtime: &Runtime) -> FunctionObject {
        let code = make_code_object(context, runtime, 0, 0, Word::NIL, Word::NIL, Word::NIL)
            .expect("code object");
        let function = make_simple_fun(context, runtime, 0, Word::NIL, Word::NIL, code)
            .expect("simple function");
        FunctionObject::try_from(function.as_word()).expect("function object")
    }

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().expect("runtime");
        let mut context = ThreadContext::new();
        context.register(&runtime).expect("context");
        super::super::register(&runtime).expect("clos registration");
        (runtime, context)
    }

    fn list(context: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
        let mut scope = ncl_object::Scope::new(context);
        let roots = scope.root_many(
            &values
                .iter()
                .copied()
                .map(ncl_object::Local::from_word)
                .collect::<Vec<_>>(),
        );
        let result = scope.make_list(runtime, &roots).expect("list");
        scope.get(result).as_word()
    }

    #[test]
    fn generic_dispatch_survives_gc_forced_on_every_allocation() {
        let (runtime, mut context) = setup();
        let package = runtime
            .find_package(&context, COMMON_LISP)
            .expect("package");
        let (mut generic, _) = Package::from_word(package)
            .intern(&mut context, &runtime, "GC-STRESS-DISPATCH")
            .expect("generic symbol");
        let class = Package::from_word(package)
            .intern(&mut context, &runtime, "INTEGER")
            .expect("class symbol")
            .0;
        let variable = Package::from_word(runtime.find_package(&context, "NCL").expect("ncl"))
            .intern(&mut context, &runtime, "VALUE")
            .expect("variable symbol")
            .0;
        let specializer = list(&mut context, &runtime, &[variable, class]);
        let mut specializers = list(&mut context, &runtime, &[specializer]);
        let mut arguments = list(&mut context, &runtime, &[Word::fixnum(7)]);
        let define = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-DEFINE-GENERIC")
                .expect("define builtin"),
        )
        .expect("define function");
        let add = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-ADD-METHOD")
                .expect("add builtin"),
        )
        .expect("add function");
        let dispatch = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-DISPATCH")
                .expect("dispatch builtin"),
        )
        .expect("dispatch function");
        let method = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "CLASS-OF")
                .expect("method builtin"),
        )
        .expect("method function");
        let mut define_word = define.as_word();
        let mut add_word = add.as_word();
        let mut dispatch_word = dispatch.as_word();
        let mut method_word = method.as_word();
        let roots = [
            push_heap_root(&runtime, &mut generic),
            push_heap_root(&runtime, &mut specializers),
            push_heap_root(&runtime, &mut arguments),
            push_heap_root(&runtime, &mut define_word),
            push_heap_root(&runtime, &mut add_word),
            push_heap_root(&runtime, &mut dispatch_word),
            push_heap_root(&runtime, &mut method_word),
        ];
        context.set_gc_stress(false);
        context.set_strict_forwarding(true);
        runtime
            .call_builtin(&mut context, define, &[generic])
            .expect("define");
        runtime
            .call_builtin(&mut context, add, &[generic, specializers, method_word])
            .expect("add");
        context.set_gc_stress(true);
        let result = runtime
            .call_builtin(&mut context, dispatch, &[generic, arguments])
            .expect("dispatch");
        let expected_class = runtime
            .class(&mut context, "INTEGER")
            .expect("class descriptor");
        let expected =
            super::super::class_name(&context, expected_class).expect("class descriptor");
        assert_eq!(
            super::super::class_name(&context, result).expect("class"),
            expected,
        );
        let _ = roots;
    }

    #[test]
    fn registered_dispatch_handles_method_qualifiers_and_missing_methods() {
        let (runtime, mut context) = setup();
        let package = runtime
            .find_package(&context, COMMON_LISP)
            .expect("package");
        let ncl_package = runtime.find_package(&context, "NCL").expect("ncl package");
        let (generic, _) = Package::from_word(package)
            .intern(&mut context, &runtime, "QUALIFIER-DISPATCH")
            .expect("generic symbol");
        let (variable, _) = Package::from_word(ncl_package)
            .intern(&mut context, &runtime, "VALUE")
            .expect("variable symbol");
        let class = Package::from_word(package)
            .intern(&mut context, &runtime, "T")
            .expect("class symbol")
            .0;
        let tag = Package::from_word(ncl_package)
            .intern(&mut context, &runtime, "*CLOS-METHOD-DEFINITION*")
            .expect("method tag")
            .0;
        let specializer = list(&mut context, &runtime, &[variable, class]);
        let specializers = list(&mut context, &runtime, &[specializer]);
        let class_of = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "CLASS-OF")
                .expect("class-of"),
        )
        .expect("class-of function");
        let define = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-DEFINE-GENERIC")
                .expect("define"),
        )
        .expect("define function");
        let add = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-ADD-METHOD")
                .expect("add"),
        )
        .expect("add function");
        let dispatch = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-DISPATCH")
                .expect("dispatch"),
        )
        .expect("dispatch function");
        runtime
            .call_builtin(&mut context, define, &[generic])
            .expect("define generic");
        runtime
            .call_builtin(
                &mut context,
                add,
                &[generic, specializers, class_of.as_word()],
            )
            .expect("primary method");
        let primary_duplicate = list(
            &mut context,
            &runtime,
            &[tag, Word::fixnum(0), specializers],
        );
        runtime
            .call_builtin(
                &mut context,
                add,
                &[generic, primary_duplicate, class_of.as_word()],
            )
            .expect("replace primary method");
        for qualifier in [1_i64, 2, 3] {
            let encoded = list(
                &mut context,
                &runtime,
                &[tag, Word::fixnum(qualifier), specializers],
            );
            runtime
                .call_builtin(&mut context, add, &[generic, encoded, class_of.as_word()])
                .expect("qualified method");
        }
        let arguments = list(&mut context, &runtime, &[Word::fixnum(7)]);
        let result = runtime
            .call_builtin(&mut context, dispatch, &[generic, arguments])
            .expect("qualified dispatch");
        let integer = runtime
            .class(&mut context, "INTEGER")
            .expect("integer class");
        let expected = super::super::class_name(&context, integer).expect("integer name");
        assert_eq!(super::super::class_name(&context, result), Ok(expected));

        let (plain_generic, _) = Package::from_word(package)
            .intern(&mut context, &runtime, "PLAIN-DISPATCH")
            .expect("plain generic");
        runtime
            .call_builtin(&mut context, define, &[plain_generic])
            .expect("define plain generic");
        let plain_arguments = list(&mut context, &runtime, &[Word::fixnum(8)]);
        assert_eq!(
            runtime.call_builtin(&mut context, dispatch, &[plain_generic, plain_arguments]),
            Err(ncl_object::ObjectError::UndefinedFunction)
        );
        let before = list(
            &mut context,
            &runtime,
            &[tag, Word::fixnum(1), specializers],
        );
        runtime
            .call_builtin(
                &mut context,
                add,
                &[plain_generic, before, class_of.as_word()],
            )
            .expect("before method");
        let before_arguments = list(&mut context, &runtime, &[Word::fixnum(8)]);
        assert_eq!(
            runtime.call_builtin(&mut context, dispatch, &[plain_generic, before_arguments]),
            Err(ncl_object::ObjectError::UndefinedFunction)
        );

        let next_method_p = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-NEXT-METHOD-P")
                .expect("next-method-p"),
        )
        .expect("next-method-p function");
        assert_eq!(
            runtime.call_builtin(&mut context, next_method_p, &[Word::NIL]),
            Ok(Word::NIL)
        );
        assert_eq!(
            runtime.call_builtin(&mut context, next_method_p, &[Word::TRUE]),
            Ok(Word::TRUE)
        );
        let call_next = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-CALL-NEXT-METHOD")
                .expect("call-next-method"),
        )
        .expect("call-next-method function");
        let supplied = list(&mut context, &runtime, &[Word::fixnum(1)]);
        assert_eq!(
            runtime.call_builtin(&mut context, call_next, &[Word::NIL, supplied]),
            Err(ncl_object::ObjectError::UndefinedFunction)
        );

        let ensure = FunctionObject::try_from(
            runtime
                .function(
                    &mut context,
                    COMMON_LISP,
                    "%CLOS-ENSURE-INITIALIZATION-BASE",
                )
                .expect("ensure initialization"),
        )
        .expect("ensure function");
        let arbitrary = Package::from_word(package)
            .intern(&mut context, &runtime, "NOT-INITIALIZATION")
            .expect("arbitrary name")
            .0;
        assert_eq!(
            runtime.call_builtin(&mut context, ensure, &[arbitrary]),
            Ok(arbitrary)
        );
        let initialize = Package::from_word(package)
            .intern(&mut context, &runtime, "INITIALIZE-INSTANCE")
            .expect("initialize name")
            .0;
        assert_eq!(
            runtime.call_builtin(&mut context, ensure, &[initialize]),
            Ok(initialize)
        );
        assert_eq!(
            runtime.call_builtin(&mut context, ensure, &[initialize]),
            Ok(initialize)
        );
    }

    #[test]
    fn call_next_method_chain_survives_gc_forced_on_every_allocation() {
        let (runtime, mut context) = setup();
        let class_of = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "CLASS-OF")
                .expect("class-of"),
        )
        .expect("class-of function");
        let next_methods = list(&mut context, &runtime, &[class_of.as_word()]);
        let supplied = list(&mut context, &runtime, &[Word::fixnum(7)]);
        let call_next = FunctionObject::try_from(
            runtime
                .function(&mut context, COMMON_LISP, "%CLOS-CALL-NEXT-METHOD")
                .expect("call-next-method"),
        )
        .expect("call-next-method function");
        let mut next_word = next_methods;
        let mut supplied_word = supplied;
        let next_token = push_heap_root(&runtime, &mut next_word);
        let supplied_token = push_heap_root(&runtime, &mut supplied_word);
        context.set_gc_stress(true);
        context.set_strict_forwarding(true);
        let mut result = runtime
            .call_builtin(&mut context, call_next, &[next_word, supplied_word])
            .expect("call-next-method");
        let result_token = push_heap_root(&runtime, &mut result);
        let integer = runtime.class(&mut context, "INTEGER").expect("integer");
        let expected = super::super::class_name(&context, integer).expect("class descriptor");
        assert_eq!(
            super::super::class_name(&context, result).expect("class"),
            expected,
        );
        let _ = (result_token, supplied_token, next_token);
    }

    #[test]
    fn method_combination_passes_next_methods_and_restores_primary_values() {
        let (runtime, mut context) = setup();
        let before = simple_function(&mut context, &runtime).as_word();
        let primary = simple_function(&mut context, &runtime).as_word();
        let next_primary = simple_function(&mut context, &runtime).as_word();
        let after = simple_function(&mut context, &runtime).as_word();
        let mut scope = Scope::new(&mut context);
        let before_values = scope.root_many(&[ncl_object::Local::from_word(before)]);
        let before_list = scope
            .make_list(&runtime, &before_values)
            .expect("before list");
        let primary_values = scope.root_many(&[
            ncl_object::Local::from_word(primary),
            ncl_object::Local::from_word(next_primary),
        ]);
        let primary_list = scope
            .make_list(&runtime, &primary_values)
            .expect("primary list");
        let after_values = scope.root_many(&[ncl_object::Local::from_word(after)]);
        let after_list = scope
            .make_list(&runtime, &after_values)
            .expect("after list");
        let method_values = scope.root_many(&[
            ncl_object::Local::from_word(Word::NIL),
            ncl_object::Local::from_word(scope.get(before_list).as_word()),
            ncl_object::Local::from_word(scope.get(primary_list).as_word()),
            ncl_object::Local::from_word(scope.get(after_list).as_word()),
        ]);
        let method = scope
            .make_simple_vector(&runtime, &method_values)
            .expect("effective method");
        let arguments = scope.root_many(&[ncl_object::Local::from_word(Word::fixnum(7))]);
        let argument_list = scope
            .make_list(&runtime, &arguments)
            .expect("argument list");
        let mut caller = RecordingCaller::new();
        let mut values = MultipleValues::new();
        let result = super::super::invoke_core(
            &mut scope,
            &runtime,
            method,
            &arguments,
            argument_list,
            &mut caller,
            &mut values,
        )
        .expect("method combination");

        assert_eq!(scope.get(result).as_word(), Word::fixnum(2));
        assert_eq!(values.as_slice(), &[Word::fixnum(2), Word::fixnum(902)]);
        assert_eq!(caller.calls.len(), 3);
        assert_eq!(caller.calls[0].0, before);
        assert_eq!(caller.calls[0].1, &[Word::NIL, Word::fixnum(7)]);
        assert_eq!(caller.calls[1].0, primary);
        assert_eq!(
            ncl_object::car(scope.context(), caller.calls[1].1[0]),
            Ok(next_primary)
        );
        assert_eq!(caller.calls[1].1[1], Word::fixnum(7));
        assert_eq!(caller.calls[2].0, after);
        assert_eq!(caller.calls[2].1, &[Word::NIL, Word::fixnum(7)]);
    }

    #[test]
    fn around_and_call_next_method_follow_marker_continuations() {
        let (runtime, mut context) = setup();
        let around = simple_function(&mut context, &runtime).as_word();
        let primary = simple_function(&mut context, &runtime).as_word();
        let mut scope = Scope::new(&mut context);
        let primary_values = scope.root_many(&[ncl_object::Local::from_word(primary)]);
        let primary_list = scope
            .make_list(&runtime, &primary_values)
            .expect("primary list");
        let marker_values = scope.root_many(&[
            ncl_object::Local::from_word(Word::NIL),
            ncl_object::Local::from_word(Word::NIL),
            ncl_object::Local::from_word(scope.get(primary_list).as_word()),
            ncl_object::Local::from_word(Word::NIL),
        ]);
        let marker = scope
            .make_simple_vector(&runtime, &marker_values)
            .expect("dispatch marker");
        let around_values = scope.root_many(&[ncl_object::Local::from_word(around)]);
        let arguments = scope.root_many(&[ncl_object::Local::from_word(Word::fixnum(9))]);
        let argument_list = scope
            .make_list(&runtime, &arguments)
            .expect("argument list");
        let no_before = scope.root_many(&[]);
        let no_after = scope.root_many(&[]);
        let mut caller = RecordingCaller::new();
        let mut values = MultipleValues::new();
        super::super::invoke_dispatch(
            &mut scope,
            &runtime,
            &no_before,
            &primary_values,
            &no_after,
            &around_values,
            &arguments,
            argument_list,
            &mut caller,
            &mut values,
        )
        .expect("around dispatch");
        assert_eq!(caller.calls.len(), 1);
        assert_eq!(caller.calls[0].0, around);
        assert!(matches!(
            ncl_object::classify_object(
                scope.context(),
                ncl_object::car(scope.context(), caller.calls[0].1[0]).unwrap()
            ),
            ncl_object::ObjectRef::SimpleVector(_)
        ));

        let continuation_values =
            scope.root_many(&[ncl_object::Local::from_word(scope.get(marker).as_word())]);
        let continuation_list = scope
            .make_list(&runtime, &continuation_values)
            .expect("marker continuation");
        super::super::invoke_continuation(
            &mut scope,
            &runtime,
            marker,
            &arguments,
            argument_list,
            &mut caller,
            &mut values,
        )
        .expect("vector continuation");
        super::super::invoke_continuation(
            &mut scope,
            &runtime,
            continuation_list,
            &arguments,
            argument_list,
            &mut caller,
            &mut values,
        )
        .expect("list marker continuation");
        let function_values = scope.root_many(&[ncl_object::Local::from_word(primary)]);
        let function_continuation = scope
            .make_list(&runtime, &function_values)
            .expect("function continuation");
        super::super::invoke_continuation(
            &mut scope,
            &runtime,
            function_continuation,
            &arguments,
            argument_list,
            &mut caller,
            &mut values,
        )
        .expect("function continuation");
        assert_eq!(caller.calls.len(), 4);
        assert_eq!(caller.calls[1].0, primary);
        assert_eq!(caller.calls[2].0, primary);
        assert_eq!(caller.calls[3].0, primary);
        assert_eq!(caller.calls[3].1[0], Word::NIL);
    }
}
