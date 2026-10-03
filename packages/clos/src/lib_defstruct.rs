include!("lib_defstruct_helpers.rs");
include!("lib_defstruct_macro.rs");

#[cfg(test)]
mod defstruct_tests {
    use super::*;

    fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word, ObjectError> {
        let mut scope = ncl_object::Scope::new(ctx);
        let roots = scope.root_many(
            &values
                .iter()
                .copied()
                .map(ncl_object::Local::from_word)
                .collect::<Vec<_>>(),
        );
        let result = scope.make_list(runtime, &roots)?;
        Ok(scope.get(result).as_word())
    }

    fn contains(ctx: &ThreadContext, value: Word, target: Word) -> Result<bool, ObjectError> {
        if value == target || !value.is_cons() {
            return Ok(value == target);
        }
        Ok(contains(ctx, ncl_object::car(ctx, value)?, target)?
            || contains(ctx, ncl_object::cdr(ctx, value)?, target)?)
    }

    #[test]
    fn boa_constructor_values_survive_gc_stress_and_strict_forwarding() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        register(&runtime)?;
        let package = runtime
            .find_package(&ctx, "COMMON-LISP-USER")
            .ok_or(ObjectError::TypeError)?;
        let intern = |name: &str, ctx: &mut ThreadContext| {
            Package::from_word(package)
                .intern(ctx, &runtime, name)
                .map(|(symbol, _)| symbol)
        };
        let defstruct = intern("DEFSTRUCT", &mut ctx)?;
        let pair = intern("PAIR", &mut ctx)?;
        let constructor = intern("CONSTRUCTOR", &mut ctx)?;
        let make_pair = intern("MAKE-PAIR", &mut ctx)?;
        let mut left = intern("LEFT", &mut ctx)?;
        let mut right = intern("RIGHT", &mut ctx)?;
        let optional = intern("&OPTIONAL", &mut ctx)?;
        let key = intern("&KEY", &mut ctx)?;
        let c = intern("C", &mut ctx)?;
        let aux = intern("&AUX", &mut ctx)?;
        let ignored = intern("IGNORED", &mut ctx)?;
        let lambda = list(
            &mut ctx,
            &runtime,
            &[left, optional, right, key, c, aux, ignored],
        )?;
        let constructor_option = list(&mut ctx, &runtime, &[constructor, make_pair, lambda])?;
        let name = list(&mut ctx, &runtime, &[pair, constructor_option])?;
        let form = list(&mut ctx, &runtime, &[defstruct, name, left, right])?;
        let mut alpha = intern("ALPHA", &mut ctx)?;
        let mut boa_list = list(
            &mut ctx,
            &runtime,
            &[Word::fixnum(1), Word::fixnum(2), Word::fixnum(3)],
        )?;
        let layout = runtime.register_structure_layout(2)?;
        let mut form = form;
        let _root = ncl_object::push_heap_root(&runtime, &mut form);
        let _left_root = ncl_object::push_heap_root(&runtime, &mut left);
        let _right_root = ncl_object::push_heap_root(&runtime, &mut right);
        let _alpha_root = ncl_object::push_heap_root(&runtime, &mut alpha);
        let _boa_list_root = ncl_object::push_heap_root(&runtime, &mut boa_list);
        let mut structure = ncl_object::make_structure(
            &mut ctx,
            &runtime,
            layout,
            &[alpha, boa_list],
        )?;
        let _structure_root = ncl_object::push_heap_root(&runtime, &mut structure);
        ctx.set_strict_forwarding(true);
        ctx.set_gc_stress(true);
        {
            let mut scope = ncl_object::Scope::new(&mut ctx);
            let lambda_root: ncl_object::Handle<'_, Word> =
                scope.root(ncl_object::Local::from_word(lambda));
            let lambda_word = scope.get(lambda_root).as_word();
            let parameters = defstruct_boa_slot_parameters(&mut scope, lambda_word)?;
            assert_eq!(parameters.len(), 3);
            assert_eq!(parameters[0].0, "LEFT");
            assert_eq!(parameters[1].0, "RIGHT");
            assert_eq!(parameters[2].0, "C");
            let left_parameter = scope.get(parameters[0].1).as_word();
            let right_parameter = scope.get(parameters[1].1).as_word();
            assert_eq!(symbol_name_string(scope.context(), left_parameter)?, "LEFT");
            assert_eq!(symbol_name_string(scope.context(), right_parameter)?, "RIGHT");
        }
        let arguments = [form];
        let args = ncl_object::BuiltinArgs::new(&arguments);
        let mut values = MultipleValues::default();
        let result = defstruct_macro_builtin(&mut ctx, &runtime, &args, &mut values)?;
        assert_eq!(ncl_object::structure_ref(&ctx, structure, 0)?, alpha);
        assert_eq!(ncl_object::structure_ref(&ctx, structure, 1)?, boa_list);
        assert!(contains(&ctx, result, left)?);
        assert!(contains(&ctx, result, right)?);
        Ok(())
    }

    #[test]
    fn inline_options_disable_generated_functions_and_read_only_setters() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        register(&runtime)?;
        let package = runtime
            .find_package(&ctx, "COMMON-LISP-USER")
            .ok_or(ObjectError::TypeError)?;
        let intern = |name: &str, ctx: &mut ThreadContext| {
            Package::from_word(package)
                .intern(ctx, &runtime, name)
                .map(|(symbol, _)| symbol)
        };
        let defstruct = intern("DEFSTRUCT", &mut ctx)?;
        let record = intern("OPTION-RECORD", &mut ctx)?;
        let predicate = intern(":PREDICATE", &mut ctx)?;
        let copier = intern(":COPIER", &mut ctx)?;
        let type_option = intern(":TYPE", &mut ctx)?;
        let structure = intern("STRUCTURE", &mut ctx)?;
        let read_only_option = intern(":READ-ONLY", &mut ctx)?;
        let value = intern("VALUE", &mut ctx)?;
        let slot = list(
            &mut ctx,
            &runtime,
            &[value, Word::NIL, read_only_option, Word::TRUE],
        )?;
        let form = list(
            &mut ctx,
            &runtime,
            &[
                defstruct,
                record,
                slot,
                predicate,
                Word::NIL,
                copier,
                Word::NIL,
                type_option,
                structure,
            ],
        )?;
        let result = defstruct_macro_builtin(
            &mut ctx,
            &runtime,
            &ncl_object::BuiltinArgs::new(&[form]),
            &mut MultipleValues::default(),
        )?;
        let predicate_function = intern("OPTION-RECORD-P", &mut ctx)?;
        let copier_function = intern("COPY-OPTION-RECORD", &mut ctx)?;
        let setter = intern("%STRUCTURE-SET", &mut ctx)?;
        let accessor = intern("OPTION-RECORD-VALUE", &mut ctx)?;
        assert!(!contains(&ctx, result, predicate_function)?);
        assert!(!contains(&ctx, result, copier_function)?);
        assert!(!contains(&ctx, result, setter)?);
        assert!(contains(&ctx, result, accessor)?);
        Ok(())
    }

    #[test]
    fn unknown_defstruct_option_is_rejected() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        register(&runtime)?;
        let package = runtime
            .find_package(&ctx, "COMMON-LISP-USER")
            .ok_or(ObjectError::TypeError)?;
        let intern = |name: &str, ctx: &mut ThreadContext| {
            Package::from_word(package)
                .intern(ctx, &runtime, name)
                .map(|(symbol, _)| symbol)
        };
        let defstruct = intern("DEFSTRUCT", &mut ctx)?;
        let record = intern("UNKNOWN-OPTION-RECORD", &mut ctx)?;
        let unknown = intern(":NOT-A-DEFSTRUCT-OPTION", &mut ctx)?;
        let form = list(
            &mut ctx,
            &runtime,
            &[defstruct, record, unknown, Word::TRUE],
        )?;
        let result = defstruct_macro_builtin(
            &mut ctx,
            &runtime,
            &ncl_object::BuiltinArgs::new(&[form]),
            &mut MultipleValues::default(),
        );
        assert_eq!(result, Err(ObjectError::TypeError));
        Ok(())
    }

    #[test]
    fn contains_rejects_absent_values_and_non_lists() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let present = Word::fixnum(1);
        let absent = Word::fixnum(2);
        assert!(!contains(&ctx, present, absent)?);
        let nested = list(&mut ctx, &runtime, &[present, Word::NIL])?;
        assert!(!contains(&ctx, nested, absent)?);
        assert!(contains(&ctx, nested, Word::NIL)?);
        Ok(())
    }
}
