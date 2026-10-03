#![allow(clippy::all, clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::similar_names, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn helper_expansions_cover_gensym_initialization_and_method_rewrites() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        register(&runtime)?;
        let mut scope = Scope::new(&mut ctx);
        let name = scope.intern(&runtime, "COMMON-LISP-USER", "INLINE-METHOD")?;
        let initialize = scope.intern(&runtime, COMMON_LISP, "INITIALIZE-INSTANCE")?;
        let quoted_name = quoted(&mut scope, &runtime, name)?;
        assert!(initialization_base(&mut scope, &runtime, name, quoted_name)?.is_none());
        let initialize_quoted = quoted(&mut scope, &runtime, initialize)?;
        assert!(initialization_base(
            &mut scope,
            &runtime,
            initialize,
            initialize_quoted,
        )?
        .is_some());

        let next_methods = gensym(&mut scope, &runtime)?;
        let value = scope.intern(&runtime, "COMMON-LISP-USER", "VALUE")?;
        let value_word = scope.get(value).as_word();
        let user_args = scope.root_many(&[ncl_object::Local::from_word(value_word)]);
        let lambda = method_lambda_list(&mut scope, &runtime, next_methods, &user_args)?;
        assert!(scope.get(lambda).as_word().is_cons());

        let quote_name = scope.intern(&runtime, COMMON_LISP, "QUOTE")?;
        let defun_name = scope.intern(&runtime, COMMON_LISP, "DEFUN")?;
        let lambda_name = scope.intern(&runtime, COMMON_LISP, "LAMBDA")?;
        let call_next_name = scope.intern(&runtime, COMMON_LISP, "CALL-NEXT-METHOD")?;
        let call_next_user_name = scope.intern(&runtime, "COMMON-LISP-USER", "CALL-NEXT-METHOD")?;
        let next_method_p_name = scope.intern(&runtime, COMMON_LISP, "NEXT-METHOD-P")?;
        let next_method_p_user_name = scope.intern(&runtime, "COMMON-LISP-USER", "NEXT-METHOD-P")?;
        let list_name = scope.intern(&runtime, COMMON_LISP, "LIST")?;
        let call_next = scope.intern(&runtime, COMMON_LISP, "%CLOS-CALL-NEXT-METHOD")?;
        let next_method_p = scope.intern(&runtime, COMMON_LISP, "%CLOS-NEXT-METHOD-P")?;
        let rewrite = MethodRewrite {
            next_methods,
            current_args: user_args.clone(),
            call_next_name,
            call_next_user_name,
            next_method_p_name,
            next_method_p_user_name,
            list_name,
            call_next,
            next_method_p,
            quote_name,
            defun_name,
            lambda_name,
        };
        let call_word = scope.get(call_next).as_word();
        let call_args = scope.root_many(&[ncl_object::Local::from_word(call_word)]);
        let call = scope.make_list(&runtime, &call_args)?;
        let next_word = scope.get(next_method_p).as_word();
        let next_args = scope.root_many(&[ncl_object::Local::from_word(next_word)]);
        let next = scope.make_list(&runtime, &next_args)?;
        let quote_word = scope.get(quote_name).as_word();
        let quoted_args = scope.root_many(&[
            ncl_object::Local::from_word(quote_word),
            ncl_object::Local::from_word(value_word),
        ]);
        let quoted = scope.make_list(&runtime, &quoted_args)?;
        let forms = scope.root_many(&[
            ncl_object::Local::from_word(scope.get(call).as_word()),
            ncl_object::Local::from_word(scope.get(next).as_word()),
            ncl_object::Local::from_word(scope.get(quoted).as_word()),
        ]);
        let direct_call = rewrite_method_form(&mut scope, &runtime, call, &rewrite)?;
        let direct_next = rewrite_method_form(&mut scope, &runtime, next, &rewrite)?;
        let direct_quoted = rewrite_method_form(&mut scope, &runtime, quoted, &rewrite)?;
        assert!(scope.get(direct_call).as_word().is_cons());
        assert!(scope.get(direct_next).as_word().is_cons());
        assert_eq!(scope.get(direct_quoted).as_word(), scope.get(quoted).as_word());
        let expanded = rewrite_method_body(&mut scope, &runtime, &forms, next_methods, &user_args)?;
        assert!(scope.get(expanded).as_word().is_cons());
        Ok(())
    }
