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
}
