use super::*;
use crate::{list, symbol};
use ncl_object::Runtime;

fn fixture() -> std::result::Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}

#[test]
#[allow(clippy::too_many_lines)]
fn parses_all_iteration_driver_shapes_and_rejects_bad_clauses()
-> std::result::Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let sequence = symbol(&mut ctx, &runtime, "SEQUENCE")?;
    let by = symbol(&mut ctx, &runtime, "NEXT")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let inputs = [
        vec![
            symbol(&mut ctx, &runtime, "FOR")?,
            x,
            symbol(&mut ctx, &runtime, "IN")?,
            sequence,
            symbol(&mut ctx, &runtime, "BY")?,
            by,
        ],
        vec![
            symbol(&mut ctx, &runtime, "AS")?,
            x,
            symbol(&mut ctx, &runtime, "ON")?,
            sequence,
        ],
        vec![
            symbol(&mut ctx, &runtime, "FOR")?,
            x,
            symbol(&mut ctx, &runtime, "ACROSS")?,
            sequence,
        ],
        vec![
            symbol(&mut ctx, &runtime, "FOR")?,
            x,
            symbol(&mut ctx, &runtime, "BEING")?,
            symbol(&mut ctx, &runtime, "EACH")?,
            symbol(&mut ctx, &runtime, "HASH-KEY")?,
            symbol(&mut ctx, &runtime, "OF")?,
            table,
        ],
        vec![
            symbol(&mut ctx, &runtime, "FOR")?,
            x,
            symbol(&mut ctx, &runtime, "FROM")?,
            Word::fixnum(3),
            symbol(&mut ctx, &runtime, "BY")?,
            Word::fixnum(2),
            symbol(&mut ctx, &runtime, "ABOVE")?,
            Word::fixnum(0),
        ],
    ];
    let asts = inputs
        .iter()
        .map(|input| parse_loop(&mut ctx, input))
        .collect::<Result<Vec<_>>>()?;
    assert!(matches!(
        asts[0].clauses[0],
        LoopClause::In {
            on: false,
            by: Some(_),
            ..
        }
    ));
    assert!(matches!(
        asts[1].clauses[0],
        LoopClause::In {
            on: true,
            by: None,
            ..
        }
    ));
    assert!(matches!(asts[2].clauses[0], LoopClause::Across { .. }));
    assert!(matches!(
        asts[3].clauses[0],
        LoopClause::Hash(HashClause {
            kind: HashIterationKind::Key,
            using: None,
            ..
        })
    ));
    assert!(matches!(
        asts[4].clauses[0],
        LoopClause::For(ForClause {
            direction: Some(StepDirection::From),
            limit: Some((LimitDirection::Above, _)),
            ..
        })
    ));
    let bad = [
        vec![symbol(&mut ctx, &runtime, "FOR")?],
        vec![
            symbol(&mut ctx, &runtime, "FOR")?,
            x,
            symbol(&mut ctx, &runtime, "BY")?,
        ],
        vec![
            symbol(&mut ctx, &runtime, "FOR")?,
            x,
            symbol(&mut ctx, &runtime, "IN")?,
            sequence,
            symbol(&mut ctx, &runtime, "BY")?,
        ],
        {
            let hash_key = symbol(&mut ctx, &runtime, "HASH-KEY")?;
            let using = list(&mut ctx, &runtime, &[hash_key, value])?;
            vec![
                symbol(&mut ctx, &runtime, "FOR")?,
                x,
                symbol(&mut ctx, &runtime, "BEING")?,
                hash_key,
                symbol(&mut ctx, &runtime, "OF")?,
                table,
                symbol(&mut ctx, &runtime, "USING")?,
                using,
            ]
        },
        vec![symbol(&mut ctx, &runtime, "UNKNOWN")?],
    ];
    for input in bad {
        assert_eq!(parse_loop(&mut ctx, &input), Err(ObjectError::TypeError));
    }
    Ok(())
}
