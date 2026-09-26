use super::*;

#[test]
#[allow(clippy::too_many_lines)]
fn all_registered_macro_expansions_survive_gc_stress_and_forwarding() -> Result<(), ObjectError> {
    fn form(ctx: &mut ThreadContext, runtime: &Runtime, name: &str, args: &[Word]) -> Result<Word, ObjectError> {
        let operator = symbol(ctx, runtime, name)?;
        let mut values = Vec::with_capacity(args.len() + 1);
        values.push(operator);
        values.extend_from_slice(args);
        list(ctx, runtime, &values)
    }

    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    register(&runtime)?;
    ctx.register(&runtime)?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let y = symbol(&mut ctx, &runtime, "Y")?;
    let t = symbol(&mut ctx, &runtime, "T")?;
    let one = Word::fixnum(1);
    let clause_keys = list(&mut ctx, &runtime, &[one])?;
    let clause = list(&mut ctx, &runtime, &[clause_keys, t])?;
    let type_clause = list(&mut ctx, &runtime, &[t, t])?;
    let do_variable = list(&mut ctx, &runtime, &[x, Word::fixnum(0), one])?;
    let do_variables = list(&mut ctx, &runtime, &[do_variable])?;
    let do_end = list(&mut ctx, &runtime, &[t, x])?;
    let prog_variable = list(&mut ctx, &runtime, &[x, Word::fixnum(0)])?;
    let prog_variables = list(&mut ctx, &runtime, &[prog_variable])?;
    let nil = Word::NIL;
    let cases = [
        ("AND", form(&mut ctx, &runtime, "AND", &[t, x])?),
        ("CASE", form(&mut ctx, &runtime, "CASE", &[x, clause])?),
        ("CCASE", form(&mut ctx, &runtime, "CCASE", &[x, clause])?),
        ("COND", form(&mut ctx, &runtime, "COND", &[clause])?),
        ("CTYPECASE", form(&mut ctx, &runtime, "CTYPECASE", &[x, type_clause])?),
        ("DECF", form(&mut ctx, &runtime, "DECF", &[x, one])?),
        ("DEFCONSTANT", form(&mut ctx, &runtime, "DEFCONSTANT", &[x, one])?),
        ("DEFINE-COMPILER-MACRO", form(&mut ctx, &runtime, "DEFMACRO", &[x, nil, t])?),
        ("DEFINE-SETF-EXPANDER", form(&mut ctx, &runtime, "DEFSETF", &[x, y])?),
        ("DEFINE-SYMBOL-MACRO", form(&mut ctx, &runtime, "DEFINE-SYMBOL-MACRO", &[x, one])?),
        ("DEFMACRO", form(&mut ctx, &runtime, "DEFMACRO", &[x, nil, t])?),
        ("DEFUN", form(&mut ctx, &runtime, "DEFUN", &[x, nil, t])?),
        ("DEFPARAMETER", form(&mut ctx, &runtime, "DEFPARAMETER", &[x, one])?),
        ("DEFSETF", form(&mut ctx, &runtime, "DEFSETF", &[x, y])?),
        ("DEFVAR", form(&mut ctx, &runtime, "DEFVAR", &[x, one])?),
        ("DO", form(&mut ctx, &runtime, "DO", &[do_variables, do_end, x])?),
        ("DO*", form(&mut ctx, &runtime, "DO*", &[do_variables, do_end, x])?),
        ("ECASE", form(&mut ctx, &runtime, "ECASE", &[x, clause])?),
        ("ETYPECASE", form(&mut ctx, &runtime, "ETYPECASE", &[x, type_clause])?),
        ("INCF", form(&mut ctx, &runtime, "INCF", &[x, one])?),
        ("NTH-VALUE", form(&mut ctx, &runtime, "NTH-VALUE", &[Word::fixnum(0), x])?),
        ("OR", form(&mut ctx, &runtime, "OR", &[x, y])?),
        ("POP", form(&mut ctx, &runtime, "POP", &[x])?),
        ("PROG", form(&mut ctx, &runtime, "PROG", &[prog_variables, x])?),
        ("PROG*", form(&mut ctx, &runtime, "PROG*", &[prog_variables, x])?),
        ("PROG1", form(&mut ctx, &runtime, "PROG1", &[x, y])?),
        ("PROG2", form(&mut ctx, &runtime, "PROG2", &[x, y])?),
        ("PSETF", form(&mut ctx, &runtime, "PSETF", &[x, one])?),
        ("PUSH", form(&mut ctx, &runtime, "PUSH", &[one, x])?),
        ("PUSHNEW", form(&mut ctx, &runtime, "PUSHNEW", &[one, x])?),
        ("REMF", form(&mut ctx, &runtime, "REMF", &[x, y])?),
        ("RETURN", form(&mut ctx, &runtime, "RETURN", &[x])?),
        ("SETF", form(&mut ctx, &runtime, "SETF", &[x, one])?),
        ("TYPECASE", form(&mut ctx, &runtime, "TYPECASE", &[x, type_clause])?),
        ("UNLESS", form(&mut ctx, &runtime, "UNLESS", &[x, y])?),
        ("WHEN", form(&mut ctx, &runtime, "WHEN", &[x, y])?),
    ];
    assert_eq!(cases.len(), MACROS.len());
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let case_words = cases.map(|(_, input)| input);
    ncl_object::with_roots(&mut ctx, &case_words, |ctx, roots| {
        for ((name, _), input_root) in cases.iter().zip(roots) {
            let mut values = ncl_object::MultipleValues::new();
            let arg_words = [**input_root];
            let args = ncl_object::BuiltinArgs::new(&arg_words);
            let expanded = callback_for(name).ok_or(ObjectError::UndefinedFunction)?(
                ctx, &runtime, &args, &mut values,
            )?;
            let mut expanded = expanded;
            let expanded_before_gc = expanded;
            let expanded_root = ncl_object::push_root(ctx, &mut expanded);
            ctx.collect(true)?;
            assert_ne!(expanded, expanded_before_gc, "{name} result did not relocate");
            assert!(!elements(ctx, expanded).unwrap_or_default().is_empty(), "{name}");
            assert!(ncl_object::pop_root(ctx, expanded_root));
        }
        Ok(())
    })
}
