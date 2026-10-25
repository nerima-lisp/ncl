use super::{expand, expand_multiple_value_setq};
use crate::{elements, list, symbol};
use ncl_object::{
    BuiltinArgs, HeapRootedWord, MultipleValues, ObjectError, Runtime, ThreadContext, Word,
};

#[test]
fn psetq_preserves_variables_and_values_under_gc_stress() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    crate::register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "PSETQ")?;
    let variable_a = symbol(&mut ctx, &runtime, "VARIABLE-A")?;
    let variable_a = HeapRootedWord::new(&ctx, variable_a)?;
    let value_a = symbol(&mut ctx, &runtime, "VALUE-A")?;
    let value_a = HeapRootedWord::new(&ctx, value_a)?;
    let variable_b = symbol(&mut ctx, &runtime, "VARIABLE-B")?;
    let variable_b = HeapRootedWord::new(&ctx, variable_b)?;
    let value_b = symbol(&mut ctx, &runtime, "VALUE-B")?;
    let value_b = HeapRootedWord::new(&ctx, value_b)?;
    let input_form = list(
        &mut ctx,
        &runtime,
        &[
            operator,
            variable_a.get(),
            value_a.get(),
            variable_b.get(),
            value_b.get(),
        ],
    )?;
    let input_form = HeapRootedWord::new(&ctx, input_form)?;
    let mut values = MultipleValues::new();
    let let_operator = symbol(&mut ctx, &runtime, "LET")?;
    let let_operator = HeapRootedWord::new(&ctx, let_operator)?;
    let setq_operator = symbol(&mut ctx, &runtime, "SETQ")?;
    let setq_operator = HeapRootedWord::new(&ctx, setq_operator)?;
    let args = [input_form.get()];
    let input = BuiltinArgs::new(&args);

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let expansion = expand(&mut ctx, &runtime, &input, &mut values)?;
    let expansion = HeapRootedWord::new(&ctx, expansion)?;
    let outer = elements(&mut ctx, expansion.get())?;
    assert_eq!(outer.first().copied(), Some(let_operator.get()));
    let bindings = *outer.get(1).ok_or(ObjectError::TypeError)?;
    let bindings = elements(&mut ctx, bindings)?;
    let first_binding = *bindings.first().ok_or(ObjectError::TypeError)?;
    let first_binding = elements(&mut ctx, first_binding)?;
    let second_binding = *bindings.get(1).ok_or(ObjectError::TypeError)?;
    let second_binding = elements(&mut ctx, second_binding)?;
    assert_eq!(first_binding.get(1), Some(&value_a.get()));
    assert_eq!(second_binding.get(1), Some(&value_b.get()));
    let assignment = *outer.get(2).ok_or(ObjectError::TypeError)?;
    let assignment = elements(&mut ctx, assignment)?;
    assert_eq!(assignment.first().copied(), Some(setq_operator.get()));
    assert_eq!(assignment.get(1), Some(&variable_a.get()));
    assert_eq!(assignment.get(3), Some(&variable_b.get()));
    assert_eq!(assignment.get(2), first_binding.first());
    assert_eq!(assignment.get(4), second_binding.first());
    Ok(())
}

#[test]
fn multiple_value_setq_without_variables_returns_the_primary_value() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "MULTIPLE-VALUE-SETQ")?;
    let value_form = symbol(&mut ctx, &runtime, "VALUE-FORM")?;
    let input_form = list(&mut ctx, &runtime, &[operator, Word::NIL, value_form])?;
    let input_form = HeapRootedWord::new(&ctx, input_form)?;
    let args = [input_form.get()];
    let input = BuiltinArgs::new(&args);
    let mut values = MultipleValues::new();
    let let_operator = symbol(&mut ctx, &runtime, "LET")?;
    let let_operator = HeapRootedWord::new(&ctx, let_operator)?;
    let prog1 = symbol(&mut ctx, &runtime, "PROG1")?;
    let prog1 = HeapRootedWord::new(&ctx, prog1)?;

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let expansion = expand_multiple_value_setq(&mut ctx, &runtime, &input, &mut values)?;
    let expansion = HeapRootedWord::new(&ctx, expansion)?;
    let outer = elements(&mut ctx, expansion.get())?;
    assert_eq!(outer.first().copied(), Some(let_operator.get()));
    let body = *outer.get(2).ok_or(ObjectError::TypeError)?;
    let body = elements(&mut ctx, body)?;
    assert_eq!(body.first().copied(), Some(prog1.get()));
    Ok(())
}

#[test]
fn multiple_value_setq_roots_variables_across_allocations() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    let operator = symbol(&mut ctx, &runtime, "MULTIPLE-VALUE-SETQ")?;
    let variable_a = symbol(&mut ctx, &runtime, "VARIABLE-A")?;
    let variable_a = HeapRootedWord::new(&ctx, variable_a)?;
    let variable_b = symbol(&mut ctx, &runtime, "VARIABLE-B")?;
    let variable_b = HeapRootedWord::new(&ctx, variable_b)?;
    let variable_list = list(&mut ctx, &runtime, &[variable_a.get(), variable_b.get()])?;
    let variable_list = HeapRootedWord::new(&ctx, variable_list)?;
    let value_form = symbol(&mut ctx, &runtime, "VALUE-FORM")?;
    let value_form = HeapRootedWord::new(&ctx, value_form)?;
    let input_form = list(
        &mut ctx,
        &runtime,
        &[operator, variable_list.get(), value_form.get()],
    )?;
    let input_form = HeapRootedWord::new(&ctx, input_form)?;
    let mut values = MultipleValues::new();
    let let_operator = symbol(&mut ctx, &runtime, "LET")?;
    let let_operator = HeapRootedWord::new(&ctx, let_operator)?;
    let prog1 = symbol(&mut ctx, &runtime, "PROG1")?;
    let prog1 = HeapRootedWord::new(&ctx, prog1)?;
    let nth = symbol(&mut ctx, &runtime, "NTH")?;
    let nth = HeapRootedWord::new(&ctx, nth)?;
    let setq = symbol(&mut ctx, &runtime, "SETQ")?;
    let setq = HeapRootedWord::new(&ctx, setq)?;
    let args = [input_form.get()];
    let input = BuiltinArgs::new(&args);

    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let expansion = expand_multiple_value_setq(&mut ctx, &runtime, &input, &mut values)?;
    let expansion = HeapRootedWord::new(&ctx, expansion)?;
    let outer = elements(&mut ctx, expansion.get())?;
    assert_eq!(outer.first().copied(), Some(let_operator.get()));
    let body = *outer.get(2).ok_or(ObjectError::TypeError)?;
    let body = elements(&mut ctx, body)?;
    assert_eq!(body.first().copied(), Some(prog1.get()));
    let assignment = *body.get(2).ok_or(ObjectError::TypeError)?;
    let assignment = elements(&mut ctx, assignment)?;
    assert_eq!(assignment.first().copied(), Some(setq.get()));
    assert_eq!(assignment.get(1), Some(&variable_a.get()));
    assert_eq!(assignment.get(3), Some(&variable_b.get()));
    let first_value = *assignment.get(2).ok_or(ObjectError::TypeError)?;
    let first_value = elements(&mut ctx, first_value)?;
    let second_value = *assignment.get(4).ok_or(ObjectError::TypeError)?;
    let second_value = elements(&mut ctx, second_value)?;
    assert_eq!(first_value.first().copied(), Some(nth.get()));
    assert_eq!(second_value.first().copied(), Some(nth.get()));
    assert_eq!(first_value.get(1), Some(&Word::fixnum(0)));
    assert_eq!(second_value.get(1), Some(&Word::fixnum(1)));
    Ok(())
}
