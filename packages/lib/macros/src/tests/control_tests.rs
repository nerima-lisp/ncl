use super::*;

fn fixture() -> Result<(Runtime, ThreadContext)> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

#[test]
fn primitive_control_expansions_preserve_values_and_modes() -> Result<()> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let y = symbol(&mut ctx, &runtime, "Y")?;
    assert_eq!(and(&mut ctx, &runtime, &[])?, Word::TRUE);
    assert_eq!(and(&mut ctx, &runtime, &[x])?, x);
    assert_eq!(or(&mut ctx, &runtime, &[])?, Word::NIL);
    assert_eq!(or(&mut ctx, &runtime, &[x])?, x);
    let condition = list(&mut ctx, &runtime, &[x, y])?;
    let cond_form = cond(&mut ctx, &runtime, &[condition])?;
    assert_eq!(
        elements(&mut ctx, cond_form)?[0],
        symbol(&mut ctx, &runtime, "IF")?
    );
    let first = prog1(&mut ctx, &runtime, &[x, y])?;
    assert_eq!(
        elements(&mut ctx, first)?[0],
        symbol(&mut ctx, &runtime, "LET")?
    );
    assert_eq!(
        nth_value(&mut ctx, &runtime, &[Word::fixnum(1), x]).map(|_| ()),
        Ok(())
    );
    let variable = list(&mut ctx, &runtime, &[x, Word::fixnum(0), y])?;
    let end = list(&mut ctx, &runtime, &[Word::TRUE, y])?;
    let variables = list(&mut ctx, &runtime, &[variable])?;
    let loop_form = do_macro(&mut ctx, &runtime, &[variables, end, x], false)?;
    assert_eq!(
        elements(&mut ctx, loop_form)?[0],
        symbol(&mut ctx, &runtime, "BLOCK")?
    );
    let forms = list(&mut ctx, &runtime, &[x])?;
    let prog_form = prog_macro(&mut ctx, &runtime, &[forms, y], true)?;
    assert_eq!(
        elements(&mut ctx, prog_form)?[0],
        symbol(&mut ctx, &runtime, "BLOCK")?
    );
    Ok(())
}

#[test]
fn named_control_expansions_select_expected_operator() -> Result<()> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let when = named(&mut ctx, &runtime, &[x, x], Kind::When)?;
    assert_eq!(
        elements(&mut ctx, when)?[0],
        symbol(&mut ctx, &runtime, "IF")?
    );
    let unless = named(&mut ctx, &runtime, &[x, x], Kind::Unless)?;
    let unless_parts = elements(&mut ctx, unless)?;
    assert_eq!(unless_parts[2], Word::NIL);
    assert_eq!(
        elements(&mut ctx, unless_parts[3])?[0],
        symbol(&mut ctx, &runtime, "PROGN")?
    );
    let return_form = named(&mut ctx, &runtime, &[x], Kind::Return)?;
    assert_eq!(
        elements(&mut ctx, return_form)?[0],
        symbol(&mut ctx, &runtime, "RETURN-FROM")?
    );
    let prog2 = named(&mut ctx, &runtime, &[x, x], Kind::Prog2)?;
    assert_eq!(
        elements(&mut ctx, prog2)?[0],
        symbol(&mut ctx, &runtime, "PROGN")?
    );
    Ok(())
}
