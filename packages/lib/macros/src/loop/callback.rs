use super::expansion::expand_loop_ast;
use super::{MultipleValues, Result, Runtime, ThreadContext, elements, parse_loop};
use ncl_object::{BuiltinArgs, ObjectError};

pub fn expand_loop_callback(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = args.get(0).ok_or(ObjectError::TypeError)?;
    let input = elements(ctx, form)?;
    let input = input.get(1..).ok_or(ObjectError::TypeError)?;
    let ast = parse_loop(ctx, input)?;
    expand_loop_ast(ctx, runtime, &ast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{list, symbol};
    use ncl_object::Word;

    #[test]
    fn callback_validates_the_form_and_returns_a_loop_expansion()
    -> std::result::Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let mut values = MultipleValues::new();
        let empty = BuiltinArgs::new(&[]);
        assert_eq!(
            expand_loop_callback(&mut ctx, &runtime, &empty, &mut values),
            Err(ObjectError::TypeError)
        );
        let x = symbol(&mut ctx, &runtime, "X")?;
        let loop_name = symbol(&mut ctx, &runtime, "LOOP")?;
        let do_name = symbol(&mut ctx, &runtime, "DO")?;
        let form = list(&mut ctx, &runtime, &[loop_name, do_name, x])?;
        let forms = [form];
        let args = BuiltinArgs::new(&forms);
        let expansion = expand_loop_callback(&mut ctx, &runtime, &args, &mut values)?;
        assert_ne!(expansion, Word::NIL);
        assert_eq!(
            ncl_object::car(&ctx, expansion)?,
            symbol(&mut ctx, &runtime, "LET")?
        );
        Ok(())
    }
}
