#[cfg(test)]
mod included_tests {
    use super::super::{COMMON_LISP, METHOD_COMBINATION_KEY_NAME};
    use ncl_object::{
        Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
        BuiltinName, BuiltinPackage, FunctionObject, LambdaList, Local, MultipleValues, Package,
        Parameter, ParameterType, Runtime, ThreadContext, Word, push_heap_root,
    };
    use std::sync::Mutex;

    const TRACE_ARGUMENT: Parameter = Parameter {
        name: BuiltinName::new("VALUE"),
        ty: ParameterType::Any,
    };
    static TRACE_TEST_LOCK: Mutex<()> = Mutex::new(());
    static METHOD_LOG: Mutex<Vec<i64>> = Mutex::new(Vec::new());

    fn record_method(event: i64) {
        METHOD_LOG
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }

    fn reset_method_log() {
        METHOD_LOG
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    fn method_log() -> Vec<i64> {
        METHOD_LOG
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    macro_rules! trace_method {
        ($name:ident, $event:expr, $result:expr, $returns:expr) => {
            fn $name(
                _ctx: &mut ThreadContext,
                _runtime: &Runtime,
                args: &BuiltinArgs<'_>,
                values: &mut MultipleValues,
            ) -> Result<Word, ncl_object::ObjectError> {
                let _ = args.required(0)?;
                record_method($event);
                let result = $result;
                values.set($returns);
                Ok(result)
            }
        };
    }

    trace_method!(trace_specific_two, 1, Word::fixnum(2), &[Word::fixnum(2)]);
    trace_method!(trace_general_three, 2, Word::fixnum(3), &[Word::fixnum(3)]);
    trace_method!(trace_false_specific, 1, Word::NIL, &[Word::NIL]);
    trace_method!(
        trace_false_specific_multi,
        1,
        Word::NIL,
        &[Word::NIL, Word::fixnum(11)]
    );
    trace_method!(
        trace_false_general_multi,
        2,
        Word::NIL,
        &[Word::NIL, Word::fixnum(24)]
    );
    trace_method!(
        trace_true_specific_multi,
        1,
        Word::TRUE,
        &[Word::TRUE, Word::fixnum(11)]
    );
    trace_method!(
        trace_true_general_multi,
        2,
        Word::TRUE,
        &[Word::TRUE, Word::fixnum(42)]
    );
    trace_method!(
        trace_progn_general_multi,
        2,
        Word::fixnum(3),
        &[Word::fixnum(3), Word::fixnum(33)]
    );
    trace_method!(
        trace_around_short_circuit,
        9,
        Word::fixnum(99),
        &[Word::fixnum(99)]
    );
    trace_method!(
        trace_standard_primary,
        2,
        Word::fixnum(17),
        &[Word::fixnum(17)]
    );
    trace_method!(trace_standard_before, 1, Word::NIL, &[Word::NIL]);
    trace_method!(trace_standard_after, 3, Word::NIL, &[Word::NIL]);

    fn trace_list(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        event: i64,
        item: i64,
        values: &mut MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        record_method(event);
        let mut scope = ncl_object::Scope::new(ctx);
        let items = scope.root_many(&[Local::from_word(Word::fixnum(item))]);
        let list = scope.make_list(runtime, &items)?;
        let result = scope.get(list).as_word();
        values.set(&[result]);
        Ok(result)
    }

    fn trace_list_one(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        let _ = args.required(0)?;
        trace_list(ctx, runtime, 1, 1, values)
    }

    fn trace_list_two(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        let _ = args.required(0)?;
        trace_list(ctx, runtime, 2, 2, values)
    }

    fn test_list_operator(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
        concatenate: bool,
    ) -> Result<Word, ncl_object::ObjectError> {
        let first = args.required(0)?;
        let second = args.required(1)?;
        let mut scope = ncl_object::Scope::new(ctx);
        let mut items = Vec::new();
        if concatenate {
            for list in [first, second] {
                let elements = scope.list_to_handle_vec(Local::from_word(list))?;
                items.extend(
                    elements
                        .iter()
                        .map(|element| scope.get(*element).as_word()),
                );
            }
        } else {
            items.extend([first, second]);
        }
        let roots = scope.root_many(
            &items
                .iter()
                .copied()
                .map(Local::from_word)
                .collect::<Vec<_>>(),
        );
        let result = scope.make_list(runtime, &roots)?;
        let result = scope.get(result).as_word();
        values.set(&[result]);
        Ok(result)
    }

    fn test_append_operator(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        test_list_operator(ctx, runtime, args, values, true)
    }

    fn test_nconc_operator(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        test_list_operator(ctx, runtime, args, values, true)
    }

    fn test_list_operator_builtin(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ncl_object::ObjectError> {
        test_list_operator(ctx, runtime, args, values, false)
    }

    fn register_trace_methods(runtime: &Runtime, context: &mut ThreadContext) {
        let callbacks: [(&str, ncl_object::RustBuiltin); 14] = [
            ("TRACE-SPECIFIC-TWO", trace_specific_two),
            ("TRACE-GENERAL-THREE", trace_general_three),
            ("TRACE-FALSE-SPECIFIC", trace_false_specific),
            ("TRACE-FALSE-SPECIFIC-MULTI", trace_false_specific_multi),
            ("TRACE-FALSE-GENERAL-MULTI", trace_false_general_multi),
            ("TRACE-TRUE-SPECIFIC-MULTI", trace_true_specific_multi),
            ("TRACE-TRUE-GENERAL-MULTI", trace_true_general_multi),
            ("TRACE-PROGN-GENERAL-MULTI", trace_progn_general_multi),
            ("TRACE-AROUND-SHORT-CIRCUIT", trace_around_short_circuit),
            ("TRACE-STANDARD-PRIMARY", trace_standard_primary),
            ("TRACE-STANDARD-BEFORE", trace_standard_before),
            ("TRACE-STANDARD-AFTER", trace_standard_after),
            ("TRACE-LIST-ONE", trace_list_one),
            ("TRACE-LIST-TWO", trace_list_two),
        ];
        for (name, callback) in callbacks {
            let implementation = BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(&[TRACE_ARGUMENT]),
                    convention: BuiltinConvention::Direct(Arity::exact(1)),
                },
                callback,
            );
            runtime
                .register_builtin(
                    context,
                    BuiltinIdentifier::new(
                        BuiltinPackage::NclTest,
                        BuiltinName::new(name),
                    ),
                    implementation,
                )
                .expect("trace method builtin");
        }
    }

    

    fn class_symbol(runtime: &Runtime, context: &mut ThreadContext, name: &str) -> Word {
        let package = runtime.find_package(context, COMMON_LISP).expect("CL package");
        Package::from_word(package)
            .intern(context, runtime, name)
            .expect("class symbol")
            .0
    }

    fn define_test_generic(runtime: &Runtime, context: &mut ThreadContext) -> Word {
        let generic = class_symbol(runtime, context, "INITIALIZE-INSTANCE");
        let package = runtime.find_package(context, "NCL").expect("NCL package");
        let variable = Package::from_word(package)
            .intern(context, runtime, "VALUE")
            .expect("argument")
            .0;
        let mut lambda_list = list(context, runtime, &[variable]);
        let _lambda_root = push_heap_root(runtime, &mut lambda_list);
        let define = FunctionObject::try_from(
            runtime
                .function(context, COMMON_LISP, "%CLOS-DEFINE-GENERIC")
                .expect("define generic"),
        )
        .expect("define generic function");
        runtime
            .call_builtin(context, define, &[generic, lambda_list])
            .expect("generic registration");
        generic
    }

    fn set_test_method_combination(
        runtime: &Runtime,
        context: &mut ThreadContext,
        generic: Word,
        combination_name: &str,
        order_name: &str,
    ) -> Result<(), ncl_object::ObjectError> {
        let mut scope = ncl_object::Scope::new(context);
        let generic = scope.root(Local::<Word>::from_word(generic));
        let key = scope.intern(runtime, "NCL", METHOD_COMBINATION_KEY_NAME)?;
        let combination = scope.intern(runtime, COMMON_LISP, combination_name)?;
        let order = scope.intern(runtime, "KEYWORD", order_name)?;
        let mut fields = scope.root_many(&[]);
        super::super::dispatch_push_handle(&mut scope, &mut fields, combination);
        super::super::dispatch_push_handle(&mut scope, &mut fields, order);
        let descriptor = scope.make_list(runtime, &fields)?;
        super::super::set_symbol_plist_property(
            &mut scope,
            runtime,
            generic,
            key,
            descriptor,
        )
    }

    fn add_test_method(
        runtime: &Runtime,
        context: &mut ThreadContext,
        generic: Word,
        class: Word,
        qualifiers: &[Word],
        function_name: &str,
    ) -> Result<(), ncl_object::ObjectError> {
        let function = runtime
            .function(context, "NCL-TEST", function_name)
            .ok_or(ncl_object::ObjectError::UndefinedFunction)?;
        let add = FunctionObject::try_from(
            runtime
                .function(context, COMMON_LISP, "%CLOS-ADD-METHOD")
                .expect("add method"),
        )
        .expect("add method function");
        let mut scope = ncl_object::Scope::new(context);
        let generic = scope.root(Local::<Word>::from_word(generic));
        let tag = scope.intern(runtime, "NCL", "*CLOS-METHOD-DEFINITION*")?;
        let qualifiers = scope.root_many(
            &qualifiers
                .iter()
                .copied()
                .map(Local::from_word)
                .collect::<Vec<_>>(),
        );
        let qualifier = qualifiers
            .as_slice()
            .first()
            .copied()
            .unwrap_or_else(|| scope.root(Local::from_word(Word::NIL)));
        let qualifier_list = scope.make_list(runtime, &qualifiers)?;
        let variable = scope.intern(runtime, "NCL", "VALUE")?;
        let class = scope.root(Local::<Word>::from_word(class));
        let mut specializer_fields = scope.root_many(&[]);
        super::super::dispatch_push_handle(&mut scope, &mut specializer_fields, variable);
        super::super::dispatch_push_handle(&mut scope, &mut specializer_fields, class);
        let specializer = scope.make_list(runtime, &specializer_fields)?;
        let mut specializers = scope.root_many(&[]);
        super::super::dispatch_push_handle(&mut scope, &mut specializers, specializer);
        let specializers = scope.make_list(runtime, &specializers)?;
        let origin = scope.root(Local::from_word(Word::NIL));
        let mut definition = scope.root_many(&[]);
        super::super::dispatch_push_handle(&mut scope, &mut definition, tag);
        super::super::dispatch_push_handle(&mut scope, &mut definition, qualifier);
        super::super::dispatch_push_handle(&mut scope, &mut definition, qualifier_list);
        super::super::dispatch_push_handle(&mut scope, &mut definition, specializers);
        super::super::dispatch_push_handle(&mut scope, &mut definition, origin);
        let encoded = scope.make_list(runtime, &definition)?;
        let mut encoded_word = scope.get(encoded).as_word();
        let _encoded_root = push_heap_root(runtime, &mut encoded_word);
        let mut generic_word = scope.get(generic).as_word();
        let _generic_root = push_heap_root(runtime, &mut generic_word);
        let mut function_word = function;
        let _function_root = push_heap_root(runtime, &mut function_word);
        drop(scope);
        runtime.call_builtin(context, add, &[generic_word, encoded_word, function_word])?;
        Ok(())
    }

    fn dispatch_with_values(
        runtime: &Runtime,
        context: &mut ThreadContext,
        generic: Word,
    ) -> Result<(Word, Vec<Word>), ncl_object::ObjectError> {
        let mut arguments = list(context, runtime, &[Word::fixnum(7)]);
        let _arguments_root = push_heap_root(runtime, &mut arguments);
        let argument_words = [generic, arguments];
        let args = BuiltinArgs::new(&argument_words);
        let mut values = MultipleValues::new();
        let result = super::super::clos_dispatch_builtin(context, runtime, &args, &mut values);
        result.map(|result| {
            if !values.is_explicit() {
                values.set(&[result]);
            }
            (result, values.as_slice().to_vec())
        })
    }

    fn list_words(context: &mut ThreadContext, value: Word) -> Vec<Word> {
        let mut scope = ncl_object::Scope::new(context);
        let values = scope
            .list_to_handle_vec(Local::from_word(value))
            .expect("list");
        values
            .iter()
            .map(|handle| scope.get(*handle).as_word())
            .collect()
    }

    fn add_two_short_primaries(
        runtime: &Runtime,
        context: &mut ThreadContext,
        generic: Word,
        combination_name: &str,
        specific_function: &str,
        general_function: &str,
    ) -> Result<(), ncl_object::ObjectError> {
        let combination = class_symbol(runtime, context, combination_name);
        let integer = class_symbol(runtime, context, "INTEGER");
        let t = class_symbol(runtime, context, "T");
        add_test_method(
            runtime,
            context,
            generic,
            integer,
            &[combination],
            specific_function,
        )?;
        add_test_method(
            runtime,
            context,
            generic,
            t,
            &[combination],
            general_function,
        )
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

    fn dispatch_matches_eql(
        context: &mut ThreadContext,
        runtime: &Runtime,
        variable: Word,
        specializer_value: Word,
        argument_value: Word,
    ) -> bool {
        let mut eql = super::super::mop::make_eql_specializer(context, runtime, specializer_value)
            .expect("eql specializer");
        let eql_root = push_heap_root(runtime, &mut eql);
        let specializer = list(context, runtime, &[variable, eql]);
        let mut specializers = list(context, runtime, &[specializer]);
        let specializers_root = push_heap_root(runtime, &mut specializers);
        let matches =
            super::super::dispatch_method_match(context, runtime, specializers, &[argument_value])
                .expect("EQL dispatch")
                .is_some();
        let _ = (eql_root, specializers_root);
        matches
    }

    fn prepare_short_generic(
        runtime: &Runtime,
        context: &mut ThreadContext,
        combination: &str,
        order: &str,
    ) -> Word {
        if runtime.function(context, COMMON_LISP, "+").is_none() {
            ncl_lib_numbers::register(runtime).expect("numeric functions");
        }
        for (name, callback) in [
            ("APPEND", test_append_operator as ncl_object::RustBuiltin),
            ("NCONC", test_nconc_operator as ncl_object::RustBuiltin),
            ("LIST", test_list_operator_builtin as ncl_object::RustBuiltin),
        ] {
            if runtime.function(context, COMMON_LISP, name).is_none() {
                let implementation = BuiltinImplementation::direct(
                    Builtin {
                        lambda_list: LambdaList::fixed(&[TRACE_ARGUMENT, TRACE_ARGUMENT]),
                        convention: BuiltinConvention::Direct(Arity::exact(2)),
                    },
                    callback,
                );
                runtime
                    .register_builtin(
                        context,
                        BuiltinIdentifier::new(
                            BuiltinPackage::CommonLisp,
                            BuiltinName::new(name),
                        ),
                        implementation,
                    )
                    .expect("sequence operator");
            }
        }
        register_trace_methods(runtime, context);
        let generic = define_test_generic(runtime, context);
        set_test_method_combination(runtime, context, generic, combination, order)
            .expect("method combination");
        generic
    }

    #[test]
    fn every_short_combination_validates_qualifiers_and_around_can_short_circuit() {
        let _lock = TRACE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for combination in [
            "+", "AND", "APPEND", "LIST", "MAX", "MIN", "NCONC", "OR", "PROGN",
        ] {
            let (runtime, mut context) = setup();
            let generic =
                prepare_short_generic(&runtime, &mut context, combination, "MOST-SPECIFIC-FIRST");
            let primary_qualifier = class_symbol(&runtime, &mut context, combination);
            let integer = class_symbol(&runtime, &mut context, "INTEGER");
            let t = class_symbol(&runtime, &mut context, "T");
            let around = scope_intern(&runtime, &mut context, "KEYWORD", "AROUND");
            add_test_method(
                &runtime,
                &mut context,
                generic,
                integer,
                &[primary_qualifier],
                "TRACE-SPECIFIC-TWO",
            )
            .expect("primary");
            add_test_method(
                &runtime,
                &mut context,
                generic,
                t,
                &[around],
                "TRACE-AROUND-SHORT-CIRCUIT",
            )
            .expect("around method");

            reset_method_log();
            let (result, returned) =
                dispatch_with_values(&runtime, &mut context, generic).expect("around result");
            assert_eq!(result, Word::fixnum(99), "{combination} around result");
            assert_eq!(returned, [Word::fixnum(99)], "{combination} around values");
            assert_eq!(method_log(), [9], "{combination} around short circuit");

            let invalid_qualifier = scope_intern(&runtime, &mut context, "KEYWORD", "BEFORE");
            add_test_method(
                &runtime,
                &mut context,
                generic,
                t,
                &[invalid_qualifier],
                "TRACE-GENERAL-THREE",
            )
            .expect("invalid qualifier method");
            reset_method_log();
            let outcome = dispatch_with_values(&runtime, &mut context, generic);
            assert!(
                matches!(outcome, Err(ncl_object::ObjectError::TypeError)),
                "{combination} invalid qualifier must signal TypeError: {outcome:?}"
            );
            assert_eq!(
                method_log(),
                [],
                "{combination} validates qualifiers before invoking methods"
            );
        }
    }

    fn scope_intern(
        runtime: &Runtime,
        context: &mut ThreadContext,
        package: &str,
        name: &str,
    ) -> Word {
        let mut scope = ncl_object::Scope::new(context);
        let value = scope.intern(runtime, package, name).expect("intern");
        scope.get(value).as_word()
    }

    #[test]
    fn multiple_short_method_qualifiers_and_missing_primary_signal_errors() {
        let _lock = TRACE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(
            &runtime,
            &mut context,
            "AND",
            "MOST-SPECIFIC-FIRST",
        );
        let and = class_symbol(&runtime, &mut context, "AND");
        let around = scope_intern(&runtime, &mut context, "KEYWORD", "AROUND");
        let integer = class_symbol(&runtime, &mut context, "INTEGER");
        let t = class_symbol(&runtime, &mut context, "T");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            integer,
            &[and],
            "TRACE-SPECIFIC-TWO",
        )
        .expect("primary");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            t,
            &[and, around],
            "TRACE-GENERAL-THREE",
        )
        .expect("multiple qualifier method");
        let outcome = dispatch_with_values(&runtime, &mut context, generic);
        assert!(
            matches!(outcome, Err(ncl_object::ObjectError::TypeError)),
            "multiple qualifiers must signal TypeError: {outcome:?}"
        );

        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(
            &runtime,
            &mut context,
            "OR",
            "MOST-SPECIFIC-FIRST",
        );
        let around = scope_intern(&runtime, &mut context, "KEYWORD", "AROUND");
        let t = class_symbol(&runtime, &mut context, "T");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            t,
            &[around],
            "TRACE-AROUND-SHORT-CIRCUIT",
        )
        .expect("around without primary");
        let outcome = dispatch_with_values(&runtime, &mut context, generic);
        assert!(
            matches!(outcome, Err(ncl_object::ObjectError::UndefinedFunction)),
            "around without primary must signal UndefinedFunction: {outcome:?}"
        );
    }

    #[test]
    fn and_and_or_short_circuit_and_preserve_multiple_values() {
        let _lock = TRACE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(
            &runtime,
            &mut context,
            "AND",
            "MOST-SPECIFIC-FIRST",
        );
        let false_specific = class_symbol(&runtime, &mut context, "INTEGER");
        let t = class_symbol(&runtime, &mut context, "T");
        let and = class_symbol(&runtime, &mut context, "AND");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            false_specific,
            &[and],
            "TRACE-FALSE-SPECIFIC-MULTI",
        )
        .expect("false primary");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            t,
            &[and],
            "TRACE-TRUE-GENERAL-MULTI",
        )
        .expect("later primary");
        reset_method_log();
        let (result, returned) =
            dispatch_with_values(&runtime, &mut context, generic).expect("and false result");
        assert_eq!(result, Word::NIL);
        assert_eq!(returned, [Word::NIL, Word::fixnum(11)]);
        assert_eq!(method_log(), [1]);

        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(
            &runtime,
            &mut context,
            "AND",
            "MOST-SPECIFIC-FIRST",
        );
        let integer = class_symbol(&runtime, &mut context, "INTEGER");
        let t = class_symbol(&runtime, &mut context, "T");
        let and = class_symbol(&runtime, &mut context, "AND");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            integer,
            &[and],
            "TRACE-TRUE-SPECIFIC-MULTI",
        )
        .expect("first true primary");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            t,
            &[and],
            "TRACE-TRUE-GENERAL-MULTI",
        )
        .expect("second true primary");
        reset_method_log();
        let (result, returned) =
            dispatch_with_values(&runtime, &mut context, generic).expect("and true result");
        assert_eq!(result, Word::TRUE);
        assert_eq!(returned, [Word::TRUE, Word::fixnum(42)]);
        assert_eq!(method_log(), [1, 2]);
    }

    #[test]
    fn or_short_circuits_and_preserves_multiple_values() {
        let _lock = TRACE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(
            &runtime,
            &mut context,
            "OR",
            "MOST-SPECIFIC-FIRST",
        );
        let integer = class_symbol(&runtime, &mut context, "INTEGER");
        let t = class_symbol(&runtime, &mut context, "T");
        let or = class_symbol(&runtime, &mut context, "OR");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            integer,
            &[or],
            "TRACE-FALSE-SPECIFIC",
        )
        .expect("false primary");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            t,
            &[or],
            "TRACE-TRUE-GENERAL-MULTI",
        )
        .expect("true primary");
        reset_method_log();
        let (result, returned) =
            dispatch_with_values(&runtime, &mut context, generic).expect("or true result");
        assert_eq!(result, Word::TRUE);
        assert_eq!(returned, [Word::TRUE, Word::fixnum(42)]);
        assert_eq!(method_log(), [1, 2]);

        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(
            &runtime,
            &mut context,
            "OR",
            "MOST-SPECIFIC-FIRST",
        );
        let integer = class_symbol(&runtime, &mut context, "INTEGER");
        let t = class_symbol(&runtime, &mut context, "T");
        let or = class_symbol(&runtime, &mut context, "OR");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            integer,
            &[or],
            "TRACE-TRUE-SPECIFIC-MULTI",
        )
        .expect("true primary");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            t,
            &[or],
            "TRACE-TRUE-GENERAL-MULTI",
        )
        .expect("unused primary");
        reset_method_log();
        let (result, returned) =
            dispatch_with_values(&runtime, &mut context, generic).expect("or short result");
        assert_eq!(result, Word::TRUE);
        assert_eq!(returned, [Word::TRUE, Word::fixnum(11)]);
        assert_eq!(method_log(), [1]);
    }

    #[test]
    fn or_all_false_preserves_last_method_multiple_values() {
        let _lock = TRACE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(
            &runtime,
            &mut context,
            "OR",
            "MOST-SPECIFIC-FIRST",
        );
        let integer = class_symbol(&runtime, &mut context, "INTEGER");
        let t = class_symbol(&runtime, &mut context, "T");
        let or = class_symbol(&runtime, &mut context, "OR");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            integer,
            &[or],
            "TRACE-FALSE-SPECIFIC",
        )
        .expect("false primary");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            t,
            &[or],
            "TRACE-FALSE-GENERAL-MULTI",
        )
        .expect("last false primary");
        reset_method_log();
        let (result, returned) =
            dispatch_with_values(&runtime, &mut context, generic).expect("or all-false result");
        assert_eq!(result, Word::NIL);
        assert_eq!(returned, [Word::NIL, Word::fixnum(24)]);
        assert_eq!(method_log(), [1, 2]);
    }

    #[test]
    fn short_operators_combine_primary_returns_and_keep_order() {
        let _lock = TRACE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (combination, expected) in [
            ("+", vec![Word::fixnum(5)]),
            ("MAX", vec![Word::fixnum(3)]),
            ("MIN", vec![Word::fixnum(2)]),
        ] {
            let (runtime, mut context) = setup();
            let generic =
                prepare_short_generic(&runtime, &mut context, combination, "MOST-SPECIFIC-FIRST");
            add_two_short_primaries(
                &runtime,
                &mut context,
                generic,
                combination,
                "TRACE-SPECIFIC-TWO",
                "TRACE-GENERAL-THREE",
            )
            .expect("numeric primaries");
            reset_method_log();
            let (result, returned) = dispatch_with_values(&runtime, &mut context, generic)
                .unwrap_or_else(|error| panic!("{combination} operator result: {error:?}"));
            assert_eq!(result, expected[0], "{combination} result");
            assert_eq!(returned, expected, "{combination} values");
            assert_eq!(method_log(), [1, 2], "{combination} primary order");
        }

        for combination in ["APPEND", "NCONC", "LIST"] {
            let (runtime, mut context) = setup();
            let generic =
                prepare_short_generic(&runtime, &mut context, combination, "MOST-SPECIFIC-FIRST");
            add_two_short_primaries(
                &runtime,
                &mut context,
                generic,
                combination,
                "TRACE-LIST-ONE",
                "TRACE-LIST-TWO",
            )
            .expect("list primaries");
            reset_method_log();
            let (result, returned) =
                dispatch_with_values(&runtime, &mut context, generic).expect("list operator");
            assert_eq!(returned, [result], "{combination} one return value");
            let values = list_words(&mut context, result);
            match combination {
                "APPEND" | "NCONC" => assert_eq!(
                    values,
                    [Word::fixnum(1), Word::fixnum(2)],
                    "{combination} concatenates method lists"
                ),
                "LIST" => {
                    assert_eq!(values.len(), 2, "LIST contains both method values");
                    assert_eq!(list_words(&mut context, values[0]), [Word::fixnum(1)]);
                    assert_eq!(list_words(&mut context, values[1]), [Word::fixnum(2)]);
                }
                _ => unreachable!(),
            }
            assert_eq!(method_log(), [1, 2], "{combination} primary order");
        }

        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(
            &runtime,
            &mut context,
            "PROGN",
            "MOST-SPECIFIC-FIRST",
        );
        add_two_short_primaries(
            &runtime,
            &mut context,
            generic,
            "PROGN",
            "TRACE-SPECIFIC-TWO",
            "TRACE-PROGN-GENERAL-MULTI",
        )
        .expect("progn primaries");
        reset_method_log();
        let (result, returned) =
            dispatch_with_values(&runtime, &mut context, generic).expect("progn result");
        assert_eq!(result, Word::fixnum(3));
        assert_eq!(returned, [Word::fixnum(3), Word::fixnum(33)]);
        assert_eq!(method_log(), [1, 2]);
    }

    #[test]
    fn most_specific_last_reverses_short_primary_method_order() {
        let _lock = TRACE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (runtime, mut context) = setup();
        let generic = prepare_short_generic(&runtime, &mut context, "PROGN", "MOST-SPECIFIC-LAST");
        add_two_short_primaries(
            &runtime,
            &mut context,
            generic,
            "PROGN",
            "TRACE-SPECIFIC-TWO",
            "TRACE-GENERAL-THREE",
        )
        .expect("progn primaries");
        reset_method_log();
        let (result, returned) =
            dispatch_with_values(&runtime, &mut context, generic).expect("last-first result");
        assert_eq!(result, Word::fixnum(2));
        assert_eq!(returned, [Word::fixnum(2)]);
        assert_eq!(method_log(), [2, 1]);
    }

    #[test]
    fn standard_method_qualifier_order_and_values_remain_unchanged() {
        let _lock = TRACE_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (runtime, mut context) = setup();
        register_trace_methods(&runtime, &mut context);
        let generic = define_test_generic(&runtime, &mut context);
        let integer = class_symbol(&runtime, &mut context, "INTEGER");
        let t = class_symbol(&runtime, &mut context, "T");
        let before = scope_intern(&runtime, &mut context, "KEYWORD", "BEFORE");
        let after = scope_intern(&runtime, &mut context, "KEYWORD", "AFTER");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            t,
            &[],
            "TRACE-STANDARD-PRIMARY",
        )
        .expect("standard primary");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            integer,
            &[before],
            "TRACE-STANDARD-BEFORE",
        )
        .expect("before method");
        add_test_method(
            &runtime,
            &mut context,
            generic,
            integer,
            &[after],
            "TRACE-STANDARD-AFTER",
        )
        .expect("after method");
        reset_method_log();
        let (result, returned) =
            dispatch_with_values(&runtime, &mut context, generic).expect("standard result");
        assert_eq!(result, Word::fixnum(17));
        assert_eq!(returned, [Word::fixnum(17)]);
        assert_eq!(method_log(), [1, 2, 3]);
    }

    #[test]
    fn generic_dispatch_survives_gc_forced_on_every_allocation() {
        let (runtime, mut context) = setup();
        let package = runtime
            .find_package(&context, COMMON_LISP)
            .expect("package");
        let (mut generic, _) = Package::from_word(package)
            .intern(&mut context, &runtime, "INITIALIZE-INSTANCE")
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
        let mut add_word = add.as_word();
        let mut dispatch_word = dispatch.as_word();
        let mut method_word = method.as_word();
        let roots = [
            push_heap_root(&runtime, &mut generic),
            push_heap_root(&runtime, &mut specializers),
            push_heap_root(&runtime, &mut arguments),
            push_heap_root(&runtime, &mut add_word),
            push_heap_root(&runtime, &mut dispatch_word),
            push_heap_root(&runtime, &mut method_word),
        ];
        context.set_strict_forwarding(true);
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
    fn dispatch_specificity_uses_argument_precedence_and_eql_descriptors() {
        let (runtime, mut context) = setup();
        let package = runtime
            .find_package(&context, COMMON_LISP)
            .expect("package");
        let (variable, _) = Package::from_word(package)
            .intern(&mut context, &runtime, "VALUE")
            .expect("variable");
        let (integer, _) = Package::from_word(package)
            .intern(&mut context, &runtime, "INTEGER")
            .expect("integer");
        let (t_class, _) = Package::from_word(package)
            .intern(&mut context, &runtime, "T")
            .expect("t class");
        let eql = super::super::mop::make_eql_specializer(&mut context, &runtime, Word::fixnum(7))
            .expect("eql specializer");
        let first_integer = list(&mut context, &runtime, &[variable, integer]);
        let first_t = list(&mut context, &runtime, &[variable, t_class]);
        let second_eql = list(&mut context, &runtime, &[variable, eql]);
        let second_t = list(&mut context, &runtime, &[variable, t_class]);
        let first = list(&mut context, &runtime, &[first_integer, second_t]);
        let second = list(&mut context, &runtime, &[first_t, second_eql]);

        let first_score = super::super::dispatch_method_match(
            &mut context,
            &runtime,
            first,
            &[Word::fixnum(7), Word::fixnum(7)],
        )
        .expect("first match")
        .expect("first applicable");
        let second_score = super::super::dispatch_method_match(
            &mut context,
            &runtime,
            second,
            &[Word::fixnum(7), Word::fixnum(7)],
        )
        .expect("second match")
        .expect("second applicable");

        let integer_class = runtime
            .class(&mut context, "INTEGER")
            .expect("integer class");
        let integer_precedence = super::super::mop::class_precedence_order(&context, integer_class)
            .expect("integer precedence");
        assert_eq!(first_score, vec![integer_precedence.len(), 0]);
        assert_eq!(second_score, vec![0, usize::MAX]);
        assert!(first_score > second_score);
    }

    #[test]
    fn eql_specializers_match_equal_bignums() {
        let (runtime, mut context) = setup();
        let package = runtime
            .find_package(&context, COMMON_LISP)
            .expect("package");
        let mut variable = Package::from_word(package)
            .intern(&mut context, &runtime, "VALUE")
            .expect("variable")
            .0;
        let variable_root = push_heap_root(&runtime, &mut variable);

        let mut specializer_value =
            ncl_object::make_bignum_from_i128(&mut context, &runtime, 1_i128 << 100)
                .expect("specializer bignum")
                .as_word();
        let specializer_value_root = push_heap_root(&runtime, &mut specializer_value);
        let mut argument_value =
            ncl_object::make_bignum_from_i128(&mut context, &runtime, 1_i128 << 100)
                .expect("argument bignum")
                .as_word();
        let argument_value_root = push_heap_root(&runtime, &mut argument_value);
        assert!(dispatch_matches_eql(
            &mut context,
            &runtime,
            variable,
            specializer_value,
            argument_value,
        ));
        let _ = (variable_root, specializer_value_root, argument_value_root);
    }

    #[test]
    fn eql_specializers_match_equal_double_floats() {
        let (runtime, mut context) = setup();
        let package = runtime
            .find_package(&context, COMMON_LISP)
            .expect("package");
        let mut variable = Package::from_word(package)
            .intern(&mut context, &runtime, "VALUE")
            .expect("variable")
            .0;
        let variable_root = push_heap_root(&runtime, &mut variable);
        let mut specializer_value = ncl_object::make_double(&mut context, &runtime, 1.5)
            .expect("specializer float")
            .as_word();
        let specializer_value_root = push_heap_root(&runtime, &mut specializer_value);
        let mut argument_value = ncl_object::make_double(&mut context, &runtime, 1.5)
            .expect("argument float")
            .as_word();
        let argument_value_root = push_heap_root(&runtime, &mut argument_value);
        assert!(dispatch_matches_eql(
            &mut context,
            &runtime,
            variable,
            specializer_value,
            argument_value,
        ));
        let _ = (variable_root, specializer_value_root, argument_value_root);
    }
}
