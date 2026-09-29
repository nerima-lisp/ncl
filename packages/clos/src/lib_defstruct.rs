include!("lib_defstruct_helpers.rs");
include!("lib_defstruct_macro.rs");

#[cfg(test)]
mod defstruct_tests {
    use super::*;

    fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
        let mut scope = ncl_object::Scope::new(ctx);
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
    fn boa_macro_expansion_survives_gc_stress_and_strict_forwarding() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context");
        register(&runtime).expect("clos registration");
        let package = runtime
            .find_package(&ctx, "COMMON-LISP-USER")
            .expect("package");
        let intern = |name: &str, ctx: &mut ThreadContext| {
            Package::from_word(package)
                .intern(ctx, &runtime, name)
                .expect("symbol")
                .0
        };
        let defstruct = intern("DEFSTRUCT", &mut ctx);
        let pair = intern("PAIR", &mut ctx);
        let constructor = intern("CONSTRUCTOR", &mut ctx);
        let make_pair = intern("MAKE-PAIR", &mut ctx);
        let left = intern("LEFT", &mut ctx);
        let right = intern("RIGHT", &mut ctx);
        let optional = intern("&OPTIONAL", &mut ctx);
        let lambda = list(&mut ctx, &runtime, &[left, optional, right]);
        let constructor_option = list(
            &mut ctx,
            &runtime,
            &[constructor, make_pair, lambda],
        );
        let name = list(&mut ctx, &runtime, &[pair, constructor_option]);
        let form = list(&mut ctx, &runtime, &[defstruct, name, left, right]);
        let mut form = form;
        let root = ncl_object::push_heap_root(&runtime, &mut form);
        ctx.set_strict_forwarding(true);
        ctx.set_gc_stress(true);
        let arguments = [form];
        let args = ncl_object::BuiltinArgs::new(&arguments);
        let mut values = MultipleValues::default();
        let result = defstruct_macro_builtin(&mut ctx, &runtime, &args, &mut values)
            .expect("defstruct macro");
        assert!(result.is_cons());
        let _ = root;
    }
}
