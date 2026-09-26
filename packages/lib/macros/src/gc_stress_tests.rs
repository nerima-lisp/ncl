use super::*;

#[test]
fn loop_clause_expansion_survives_gc_stress_and_strict_forwarding() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    register(&runtime)?;
    ctx.register(&runtime)?;
    ctx.set_strict_forwarding(true);

    let named = symbol(&mut ctx, &runtime, "LOOP-BLOCK")?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let y = symbol(&mut ctx, &runtime, "Y")?;
    let z = symbol(&mut ctx, &runtime, "Z")?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let do_form = list(&mut ctx, &runtime, &[x])?;
    let initial_form = list(&mut ctx, &runtime, &[y])?;
    let final_form = list(&mut ctx, &runtime, &[z])?;
    let ast = r#loop::LoopAst {
        name: Some(named),
        clauses: vec![
            r#loop::LoopClause::With {
                variable: x,
                init: Word::fixnum(0),
            },
            r#loop::LoopClause::For(r#loop::ForClause {
                variable: y,
                init: Word::fixnum(0),
                step: Some(Word::fixnum(1)),
                direction: Some(r#loop::StepDirection::UpFrom),
                limit: Some((r#loop::LimitDirection::Below, Word::fixnum(2))),
            }),
            r#loop::LoopClause::Hash(r#loop::HashClause {
                variable: key,
                kind: r#loop::HashIterationKind::Key,
                table,
                using: Some((r#loop::HashIterationKind::Value, value)),
            }),
            r#loop::LoopClause::Repeat(Word::fixnum(1)),
            r#loop::LoopClause::While(x),
            r#loop::LoopClause::Until(y),
            r#loop::LoopClause::Initially(vec![initial_form]),
            r#loop::LoopClause::Finally(vec![final_form]),
            r#loop::LoopClause::Do(vec![do_form]),
            r#loop::LoopClause::Accumulate {
                kind: r#loop::AccumulatorKind::Collect,
                form: x,
                variable: None,
            },
            r#loop::LoopClause::Accumulate {
                kind: r#loop::AccumulatorKind::Append,
                form: x,
                variable: None,
            },
            r#loop::LoopClause::Accumulate {
                kind: r#loop::AccumulatorKind::Nconc,
                form: x,
                variable: None,
            },
            r#loop::LoopClause::Accumulate {
                kind: r#loop::AccumulatorKind::Count,
                form: x,
                variable: None,
            },
            r#loop::LoopClause::Accumulate {
                kind: r#loop::AccumulatorKind::Sum,
                form: x,
                variable: None,
            },
            r#loop::LoopClause::Accumulate {
                kind: r#loop::AccumulatorKind::Maximize,
                form: x,
                variable: None,
            },
            r#loop::LoopClause::Accumulate {
                kind: r#loop::AccumulatorKind::Minimize,
                form: x,
                variable: None,
            },
            r#loop::LoopClause::Return(z),
        ],
    };
    ctx.set_gc_stress(true);
    let mut expansion = r#loop::expand_loop_ast(&mut ctx, &runtime, &ast)?;
    let before = expansion;
    let root = ncl_object::push_root(&mut ctx, &mut expansion);
    ctx.collect(true)?;
    // check-added-lines: allow(panic) test-only assertion
    assert_ne!(expansion, before);
    // check-added-lines: allow(panic) test-only assertion
    assert!(!elements(&mut ctx, expansion)?.is_empty());
    // check-added-lines: allow(panic) test-only assertion
    assert!(ncl_object::pop_root(&mut ctx, root));
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn all_registered_macro_expansions_survive_gc_stress_and_forwarding() -> Result<(), ObjectError> {
    fn form(
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: &str,
        args: &[Word],
    ) -> Result<Word, ObjectError> {
        let mut operator = symbol(ctx, runtime, name)?;
        ncl_object::with_root(ctx, &mut operator, |ctx, operator| {
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(*operator);
            values.extend_from_slice(args);
            list(ctx, runtime, &values)
        })
    }

    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    register(&runtime)?;
    ctx.register(&runtime)?;
    ctx.set_strict_forwarding(true);

    let mut roots = Vec::new();
    macro_rules! root {
        ($value:expr) => {
            roots.push(ncl_object::push_root(&mut ctx, $value));
        };
    }

    let mut x = symbol(&mut ctx, &runtime, "X")?;
    root!(&mut x);
    let mut y = symbol(&mut ctx, &runtime, "Y")?;
    root!(&mut y);
    let mut t = symbol(&mut ctx, &runtime, "T")?;
    root!(&mut t);
    let one = Word::fixnum(1);
    let mut clause_keys = list(&mut ctx, &runtime, &[one])?;
    root!(&mut clause_keys);
    let mut clause = list(&mut ctx, &runtime, &[clause_keys, t])?;
    root!(&mut clause);
    let mut type_clause = list(&mut ctx, &runtime, &[t, t])?;
    root!(&mut type_clause);
    let mut do_variable = list(&mut ctx, &runtime, &[x, Word::fixnum(0), one])?;
    root!(&mut do_variable);
    let mut do_variables = list(&mut ctx, &runtime, &[do_variable])?;
    root!(&mut do_variables);
    let mut do_end = list(&mut ctx, &runtime, &[t, x])?;
    root!(&mut do_end);
    let mut prog_variable = list(&mut ctx, &runtime, &[x, Word::fixnum(0)])?;
    root!(&mut prog_variable);
    let mut prog_variables = list(&mut ctx, &runtime, &[prog_variable])?;
    root!(&mut prog_variables);
    let mut destructuring_lambda_list = list(&mut ctx, &runtime, &[x])?;
    root!(&mut destructuring_lambda_list);
    let mut dolist_spec = list(&mut ctx, &runtime, &[x, Word::NIL])?;
    root!(&mut dolist_spec);
    let mut dotimes_spec = list(&mut ctx, &runtime, &[x, one])?;
    root!(&mut dotimes_spec);
    let mut for_keyword = symbol(&mut ctx, &runtime, "FOR")?;
    root!(&mut for_keyword);
    let mut from_keyword = symbol(&mut ctx, &runtime, "FROM")?;
    root!(&mut from_keyword);
    let mut to_keyword = symbol(&mut ctx, &runtime, "TO")?;
    root!(&mut to_keyword);
    let mut do_keyword = symbol(&mut ctx, &runtime, "DO")?;
    root!(&mut do_keyword);
    let nil = Word::NIL;
    let mut cases: Vec<(&str, Box<Word>)> = Vec::with_capacity(MACROS.len());
    macro_rules! case {
        ($name:literal, $operator:literal, [$($arg:expr),* $(,)?]) => {{
            let input = form(&mut ctx, &runtime, $operator, &[$($arg),*])?;
            cases.push(($name, Box::new(input)));
            let case_slot = cases.last_mut().ok_or(ObjectError::TypeError)?.1.as_mut();
            root!(case_slot);
        }};
    }
    case!("AND", "AND", [t, x]);
    case!("CASE", "CASE", [x, clause]);
    case!("CCASE", "CCASE", [x, clause]);
    case!("COND", "COND", [clause]);
    case!("CTYPECASE", "CTYPECASE", [x, type_clause]);
    case!("DECF", "DECF", [x, one]);
    case!("DEFCONSTANT", "DEFCONSTANT", [x, one]);
    case!("DEFINE-COMPILER-MACRO", "DEFMACRO", [x, nil, t]);
    case!("DEFINE-SETF-EXPANDER", "DEFSETF", [x, y]);
    case!("DEFINE-SYMBOL-MACRO", "DEFINE-SYMBOL-MACRO", [x, one]);
    case!("DEFMACRO", "DEFMACRO", [x, nil, t]);
    case!("DEFUN", "DEFUN", [x, nil, t]);
    case!("DEFPARAMETER", "DEFPARAMETER", [x, one]);
    case!("DEFSETF", "DEFSETF", [x, y]);
    case!("DEFVAR", "DEFVAR", [x, one]);
    case!("DESTRUCTURING-BIND", "DESTRUCTURING-BIND", [destructuring_lambda_list, x, x]);
    case!("DO", "DO", [do_variables, do_end, x]);
    case!("DO*", "DO*", [do_variables, do_end, x]);
    case!("DOLIST", "DOLIST", [dolist_spec, x]);
    case!("DOTIMES", "DOTIMES", [dotimes_spec, x]);
    case!("ECASE", "ECASE", [x, clause]);
    case!("ETYPECASE", "ETYPECASE", [x, type_clause]);
    case!("INCF", "INCF", [x, one]);
    case!("LOOP", "LOOP", [for_keyword, x, from_keyword, one, to_keyword, one, do_keyword, x]);
    case!("NTH-VALUE", "NTH-VALUE", [Word::fixnum(0), x]);
    case!("OR", "OR", [x, y]);
    case!("POP", "POP", [x]);
    case!("PROG", "PROG", [prog_variables, x]);
    case!("PROG*", "PROG*", [prog_variables, x]);
    case!("PROG1", "PROG1", [x, y]);
    case!("PROG2", "PROG2", [x, y]);
    case!("PSETF", "PSETF", [x, one]);
    case!("PUSH", "PUSH", [one, x]);
    case!("PUSHNEW", "PUSHNEW", [one, x]);
    case!("REMF", "REMF", [x, y]);
    case!("RETURN", "RETURN", [x]);
    case!("SETF", "SETF", [x, one]);
    case!("TYPECASE", "TYPECASE", [x, type_clause]);
    case!("UNLESS", "UNLESS", [x, y]);
    case!("WHEN", "WHEN", [x, y]);
    assert_eq!(cases.len(), MACROS.len());
    ctx.set_gc_stress(true);
    let case_words: Vec<_> = cases.iter().map(|(_, input)| **input).collect();
    let result = ncl_object::with_roots(&mut ctx, &case_words, |ctx, roots| {
        for ((name, _), input_root) in cases.iter().zip(roots) {
            let mut values = ncl_object::MultipleValues::new();
            let arg_words = [**input_root];
            let args = ncl_object::BuiltinArgs::new(&arg_words);
            let expanded = callback_for(name).ok_or(ObjectError::UndefinedFunction)?(
                ctx,
                &runtime,
                &args,
                &mut values,
            )?;
            let mut expanded = expanded;
            let expanded_before_gc = expanded;
            let expanded_root = ncl_object::push_root(ctx, &mut expanded);
            ctx.collect(true)?;
            assert_ne!(
                expanded, expanded_before_gc,
                "{name} result did not relocate"
            );
            let expanded_elements = elements(ctx, expanded)?;
            assert!(!expanded_elements.is_empty(), "{name}");
            assert!(ncl_object::pop_root(ctx, expanded_root));
        }
        Ok(())
    });
    for token in roots.into_iter().rev() {
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
    result
}
