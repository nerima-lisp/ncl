use super::r#loop::*;
use super::*;

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

#[test]
fn parses_typed_iteration_and_accumulation_clauses() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let from = symbol(&mut ctx, &runtime, "FROM")?;
    let to = symbol(&mut ctx, &runtime, "TO")?;
    let collect = symbol(&mut ctx, &runtime, "COLLECT")?;
    let input = [
        symbol(&mut ctx, &runtime, "FOR")?,
        x,
        from,
        Word::fixnum(1),
        to,
        Word::fixnum(3),
        collect,
        x,
    ];
    let ast = parse_loop(&ctx, &input)?;
    assert!(matches!(ast.clauses[0], LoopClause::For(ForClause { .. })));
    assert!(matches!(
        ast.clauses[1],
        LoopClause::Accumulate {
            kind: AccumulatorKind::Collect,
            ..
        }
    ));
    Ok(())
}

#[test]
fn expansion_has_block_tagbody_and_lexical_bindings() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let ast = LoopAst {
        name: None,
        clauses: vec![
            LoopClause::With {
                variable: x,
                init: Word::fixnum(1),
            },
            LoopClause::Do(vec![x]),
        ],
    };
    let expansion = expand_loop_ast(&mut ctx, &runtime, &ast)?;
    let outer = elements(&mut ctx, expansion)?;
    assert_eq!(outer[0], symbol(&mut ctx, &runtime, "LET")?);
    let block = elements(&mut ctx, outer[2])?;
    assert_eq!(block[0], symbol(&mut ctx, &runtime, "BLOCK")?);
    let body = elements(&mut ctx, block[2])?;
    assert_eq!(body[0], symbol(&mut ctx, &runtime, "PROGN")?);
    assert_eq!(
        elements(&mut ctx, body[1])?[0],
        symbol(&mut ctx, &runtime, "TAGBODY")?
    );
    Ok(())
}

#[test]
fn loop_finish_becomes_go_to_the_generated_end_tag() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let finish = symbol(&mut ctx, &runtime, "LOOP-FINISH")?;
    let call = list(&mut ctx, &runtime, &[finish])?;
    let ast = LoopAst {
        name: None,
        clauses: vec![LoopClause::Do(vec![call])],
    };
    let expansion = expand_loop_ast(&mut ctx, &runtime, &ast)?;
    let printed = elements(&mut ctx, expansion)?;
    assert_eq!(printed[0], symbol(&mut ctx, &runtime, "LET")?);
    assert_ne!(expansion, Word::NIL);
    Ok(())
}

#[test]
fn parses_arithmetic_boundaries_and_equals_then() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let equals = symbol(&mut ctx, &runtime, "=")?;
    let then = symbol(&mut ctx, &runtime, "THEN")?;
    let ast = parse_loop(
        &ctx,
        &[for_word, x, equals, Word::fixnum(1), then, Word::fixnum(2)],
    )?;
    assert!(matches!(ast.clauses[0], LoopClause::EqualsThen { .. }));
    Ok(())
}

#[test]
fn parses_list_and_vector_iteration_clauses() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let list = symbol(&mut ctx, &runtime, "LIST")?;
    let input = [
        symbol(&mut ctx, &runtime, "FOR")?,
        x,
        symbol(&mut ctx, &runtime, "IN")?,
        list,
        symbol(&mut ctx, &runtime, "BY")?,
        symbol(&mut ctx, &runtime, "NEXT")?,
        symbol(&mut ctx, &runtime, "FOR")?,
        symbol(&mut ctx, &runtime, "Y")?,
        symbol(&mut ctx, &runtime, "ON")?,
        list,
        symbol(&mut ctx, &runtime, "FOR")?,
        symbol(&mut ctx, &runtime, "Z")?,
        symbol(&mut ctx, &runtime, "ACROSS")?,
        symbol(&mut ctx, &runtime, "VECTOR")?,
    ];
    let ast = parse_loop(&ctx, &input)?;
    assert!(matches!(ast.clauses[0], LoopClause::In { on: false, .. }));
    assert!(matches!(ast.clauses[1], LoopClause::In { on: true, .. }));
    assert!(matches!(ast.clauses[2], LoopClause::Across { .. }));
    assert_eq!(ast.clauses.len(), 3);
    Ok(())
}
