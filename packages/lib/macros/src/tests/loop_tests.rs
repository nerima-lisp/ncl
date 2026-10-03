use super::r#loop::*;
use super::*;

fn fixture() -> Result<(Runtime, ThreadContext), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    Ok((runtime, ctx))
}
fn contains_word(ctx: &mut ThreadContext, form: Word, needle: Word) -> Result<bool, ObjectError> {
    if form == needle || !form.is_cons() {
        return Ok(form == needle);
    }
    let mut cursor = form;
    loop {
        if !cursor.is_cons() {
            return if cursor == Word::NIL {
                Ok(false)
            } else {
                contains_word(ctx, cursor, needle)
            };
        }
        if contains_word(ctx, ncl_object::car(ctx, cursor)?, needle)? {
            return Ok(true);
        }
        cursor = ncl_object::cdr(ctx, cursor)?;
    }
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
    let ast = parse_loop(&mut ctx, &input)?;
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
fn parses_control_clauses_and_non_collect_accumulators() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let input = [
        symbol(&mut ctx, &runtime, "WITH")?,
        symbol(&mut ctx, &runtime, "X")?,
        symbol(&mut ctx, &runtime, "=")?,
        Word::fixnum(1),
        symbol(&mut ctx, &runtime, "REPEAT")?,
        Word::fixnum(2),
        symbol(&mut ctx, &runtime, "WHILE")?,
        symbol(&mut ctx, &runtime, "PREDICATE")?,
        symbol(&mut ctx, &runtime, "UNTIL")?,
        symbol(&mut ctx, &runtime, "DONE")?,
        symbol(&mut ctx, &runtime, "INITIALLY")?,
        symbol(&mut ctx, &runtime, "START")?,
        symbol(&mut ctx, &runtime, "FINALLY")?,
        symbol(&mut ctx, &runtime, "STOP")?,
        symbol(&mut ctx, &runtime, "APPEND")?,
        symbol(&mut ctx, &runtime, "ITEMS")?,
        symbol(&mut ctx, &runtime, "INTO")?,
        symbol(&mut ctx, &runtime, "APPENDED")?,
        symbol(&mut ctx, &runtime, "SUM")?,
        symbol(&mut ctx, &runtime, "X")?,
        symbol(&mut ctx, &runtime, "INTO")?,
        symbol(&mut ctx, &runtime, "TOTAL")?,
        symbol(&mut ctx, &runtime, "MAXIMIZE")?,
        symbol(&mut ctx, &runtime, "X")?,
        symbol(&mut ctx, &runtime, "INTO")?,
        symbol(&mut ctx, &runtime, "HIGHEST")?,
        symbol(&mut ctx, &runtime, "MINIMIZE")?,
        symbol(&mut ctx, &runtime, "X")?,
        symbol(&mut ctx, &runtime, "INTO")?,
        symbol(&mut ctx, &runtime, "LOWEST")?,
    ];
    let ast = parse_loop(&mut ctx, &input)?;
    assert!(matches!(ast.clauses[0], LoopClause::With { .. }));
    assert!(matches!(ast.clauses[1], LoopClause::Repeat(_)));
    assert!(matches!(ast.clauses[2], LoopClause::While(_)));
    assert!(matches!(ast.clauses[3], LoopClause::Until(_)));
    assert!(matches!(&ast.clauses[4], LoopClause::Initially(forms) if forms.len() == 1));
    assert!(matches!(&ast.clauses[5], LoopClause::Finally(forms) if forms.len() == 1));
    assert!(matches!(
        ast.clauses[6],
        LoopClause::Accumulate {
            kind: AccumulatorKind::Append,
            variable: Some(_),
            ..
        }
    ));
    assert!(matches!(
        ast.clauses[7],
        LoopClause::Accumulate {
            kind: AccumulatorKind::Sum,
            variable: Some(_),
            ..
        }
    ));
    assert!(matches!(
        ast.clauses[8],
        LoopClause::Accumulate {
            kind: AccumulatorKind::Maximize,
            variable: Some(_),
            ..
        }
    ));
    assert!(matches!(
        ast.clauses[9],
        LoopClause::Accumulate {
            kind: AccumulatorKind::Minimize,
            variable: Some(_),
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
        &mut ctx,
        &[for_word, x, equals, Word::fixnum(1), then, Word::fixnum(2)],
    )?;
    assert!(matches!(ast.clauses[0], LoopClause::EqualsThen { .. }));
    Ok(())
}

#[test]
fn parses_nested_conditionals_with_else_and_rejects_empty_selectable_clause()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let test = symbol(&mut ctx, &runtime, "TEST")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let alternate = symbol(&mut ctx, &runtime, "ALTERNATE")?;
    let nested_test = symbol(&mut ctx, &runtime, "NESTED-TEST")?;
    let input = [
        symbol(&mut ctx, &runtime, "WHEN")?,
        test,
        symbol(&mut ctx, &runtime, "DO")?,
        body,
        symbol(&mut ctx, &runtime, "AND")?,
        symbol(&mut ctx, &runtime, "RETURN")?,
        alternate,
        symbol(&mut ctx, &runtime, "ELSE")?,
        symbol(&mut ctx, &runtime, "UNLESS")?,
        nested_test,
        symbol(&mut ctx, &runtime, "DO")?,
        alternate,
        symbol(&mut ctx, &runtime, "END")?,
        symbol(&mut ctx, &runtime, "END")?,
    ];
    let ast = parse_loop(&mut ctx, &input)?;
    assert!(matches!(
        ast.clauses.as_slice(),
        [LoopClause::Conditional {
            kind: ConditionalKind::When,
            then,
            otherwise,
            ..
        }] if then.len() == 2 && matches!(otherwise.as_slice(), [LoopClause::Conditional {
            kind: ConditionalKind::Unless,
            then,
            otherwise: nested_otherwise,
            ..
        }] if then.len() == 1 && nested_otherwise.is_empty())
    ));
    let malformed = [
        symbol(&mut ctx, &runtime, "WHEN")?,
        test,
        symbol(&mut ctx, &runtime, "END")?,
    ];
    assert_eq!(
        parse_loop(&mut ctx, &malformed),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn expands_unless_with_then_and_else_progns() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let test = symbol(&mut ctx, &runtime, "TEST")?;
    let then_form = symbol(&mut ctx, &runtime, "THEN-FORM")?;
    let else_form = symbol(&mut ctx, &runtime, "ELSE-FORM")?;
    let ast = LoopAst {
        name: None,
        clauses: vec![LoopClause::Conditional {
            kind: ConditionalKind::Unless,
            test,
            then: vec![LoopClause::Do(vec![then_form])],
            otherwise: vec![LoopClause::Do(vec![else_form])],
        }],
    };
    let expansion = expand_loop_ast(&mut ctx, &runtime, &ast)?;
    let outer = elements(&mut ctx, expansion)?;
    let block = elements(&mut ctx, outer[2])?;
    let block_body = elements(&mut ctx, block[2])?;
    let tagbody = elements(&mut ctx, block_body[1])?;
    let let_symbol = symbol(&mut ctx, &runtime, "LET")?;
    let let_form = tagbody
        .iter()
        .copied()
        .find_map(|form| {
            let parts = elements(&mut ctx, form).ok()?;
            (parts.first() == Some(&let_symbol)).then_some(parts)
        })
        .ok_or(ObjectError::TypeError)?;
    assert_eq!(let_form[0], symbol(&mut ctx, &runtime, "LET")?);
    let if_form = elements(&mut ctx, let_form[2])?;
    assert_eq!(if_form[0], symbol(&mut ctx, &runtime, "IF")?);
    let condition = elements(&mut ctx, if_form[1])?;
    assert_eq!(condition[0], symbol(&mut ctx, &runtime, "NOT")?);
    assert_eq!(
        elements(&mut ctx, if_form[2])?[0],
        symbol(&mut ctx, &runtime, "PROGN")?
    );
    assert_eq!(
        elements(&mut ctx, if_form[3])?[0],
        symbol(&mut ctx, &runtime, "PROGN")?
    );
    Ok(())
}

#[test]
fn expands_control_clauses_accumulators_and_if_with_else() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let items = symbol(&mut ctx, &runtime, "ITEMS")?;
    let appended = symbol(&mut ctx, &runtime, "APPENDED")?;
    let total = symbol(&mut ctx, &runtime, "TOTAL")?;
    let highest = symbol(&mut ctx, &runtime, "HIGHEST")?;
    let lowest = symbol(&mut ctx, &runtime, "LOWEST")?;
    let initial = symbol(&mut ctx, &runtime, "INITIAL-FORM")?;
    let final_form = symbol(&mut ctx, &runtime, "FINAL-FORM")?;
    let then_form = symbol(&mut ctx, &runtime, "THEN-FORM")?;
    let else_form = symbol(&mut ctx, &runtime, "ELSE-FORM")?;
    let predicate = symbol(&mut ctx, &runtime, "PREDICATE")?;
    let done = symbol(&mut ctx, &runtime, "DONE")?;
    let test = symbol(&mut ctx, &runtime, "TEST")?;
    let expansion = expand_loop_ast(
        &mut ctx,
        &runtime,
        &LoopAst {
            name: None,
            clauses: vec![
                LoopClause::With {
                    variable: x,
                    init: Word::fixnum(1),
                },
                LoopClause::Repeat(Word::fixnum(2)),
                LoopClause::While(predicate),
                LoopClause::Until(done),
                LoopClause::Initially(vec![initial]),
                LoopClause::Finally(vec![final_form]),
                LoopClause::Accumulate {
                    kind: AccumulatorKind::Append,
                    form: items,
                    variable: Some(appended),
                },
                LoopClause::Accumulate {
                    kind: AccumulatorKind::Sum,
                    form: x,
                    variable: Some(total),
                },
                LoopClause::Accumulate {
                    kind: AccumulatorKind::Maximize,
                    form: x,
                    variable: Some(highest),
                },
                LoopClause::Accumulate {
                    kind: AccumulatorKind::Minimize,
                    form: x,
                    variable: Some(lowest),
                },
                LoopClause::Conditional {
                    kind: ConditionalKind::If,
                    test,
                    then: vec![LoopClause::Do(vec![then_form])],
                    otherwise: vec![LoopClause::Do(vec![else_form])],
                },
            ],
        },
    )?;
    for operator_name in ["<=", "NOT", "APPEND", "INCF", "MAX", "MIN", "IF"] {
        let operator = symbol(&mut ctx, &runtime, operator_name)?;
        assert!(
            contains_word(&mut ctx, expansion, operator)?,
            "missing {operator_name}"
        );
    }
    for form in [initial, final_form, then_form, else_form] {
        assert!(contains_word(&mut ctx, expansion, form)?);
    }
    let outer = elements(&mut ctx, expansion)?;
    let block = elements(&mut ctx, outer[2])?;
    let block_body = elements(&mut ctx, block[2])?;
    assert_eq!(block_body.len(), 5);
    let initial_form = symbol(&mut ctx, &runtime, "INITIAL-FORM")?;
    let final_form = symbol(&mut ctx, &runtime, "FINAL-FORM")?;
    assert!(contains_word(&mut ctx, block_body[1], initial_form)?);
    assert!(contains_word(&mut ctx, block_body[3], final_form)?);
    assert!(contains_word(&mut ctx, expansion, predicate)?);
    assert!(contains_word(&mut ctx, expansion, done)?);
    Ok(())
}

#[test]
fn expands_return_and_accumulators_to_expected_result_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let returned = symbol(&mut ctx, &runtime, "RETURNED")?;
    let loop_name = symbol(&mut ctx, &runtime, "NAMED-LOOP")?;
    let return_expansion = expand_loop_ast(
        &mut ctx,
        &runtime,
        &LoopAst {
            name: Some(loop_name),
            clauses: vec![LoopClause::Return(returned)],
        },
    )?;
    let return_from = symbol(&mut ctx, &runtime, "RETURN-FROM")?;
    assert!(contains_word(&mut ctx, return_expansion, return_from)?);
    assert!(contains_word(&mut ctx, return_expansion, loop_name)?);
    assert!(contains_word(&mut ctx, return_expansion, returned)?);

    let cases = [
        (
            AccumulatorKind::Collect,
            "COLLECTED",
            "PUSH",
            Some("NREVERSE"),
        ),
        (AccumulatorKind::Append, "APPENDED", "APPEND", None),
        (AccumulatorKind::Nconc, "CONCATENATED", "NCONC", None),
        (AccumulatorKind::Count, "COUNTED", "WHEN", None),
        (AccumulatorKind::Sum, "TOTAL", "INCF", None),
        (AccumulatorKind::Maximize, "HIGHEST", "MAX", None),
        (AccumulatorKind::Minimize, "LOWEST", "MIN", None),
    ];
    for (kind, variable_name, update_operator, result_operator) in cases {
        let variable = symbol(&mut ctx, &runtime, variable_name)?;
        let value = symbol(&mut ctx, &runtime, "VALUE")?;
        let expansion = expand_loop_ast(
            &mut ctx,
            &runtime,
            &LoopAst {
                name: None,
                clauses: vec![LoopClause::Accumulate {
                    kind,
                    form: value,
                    variable: Some(variable),
                }],
            },
        )?;
        let update_operator = symbol(&mut ctx, &runtime, update_operator)?;
        assert!(contains_word(&mut ctx, expansion, update_operator)?);
        let outer = elements(&mut ctx, expansion)?;
        let block = elements(&mut ctx, outer[2])?;
        let block_body = elements(&mut ctx, block[2])?;
        let result = block_body.last().copied().ok_or(ObjectError::TypeError)?;
        if let Some(operator_name) = result_operator {
            let result_form = elements(&mut ctx, result)?;
            let result_operator = symbol(&mut ctx, &runtime, operator_name)?;
            assert_eq!(result_form[0], result_operator);
            assert_eq!(result_form[1], variable);
        } else {
            assert_eq!(result, variable);
        }
    }
    Ok(())
}

#[test]
fn parses_return_and_nconc_count_clauses_without_merging_their_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let returned = symbol(&mut ctx, &runtime, "RETURNED")?;
    let concatenated = symbol(&mut ctx, &runtime, "CONCATENATED")?;
    let counted = symbol(&mut ctx, &runtime, "COUNTED")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let return_word = symbol(&mut ctx, &runtime, "RETURN")?;
    let nconc = symbol(&mut ctx, &runtime, "NCONC")?;
    let into = symbol(&mut ctx, &runtime, "INTO")?;
    let count = symbol(&mut ctx, &runtime, "COUNT")?;
    let ast = parse_loop(
        &mut ctx,
        &[
            return_word,
            returned,
            nconc,
            value,
            into,
            concatenated,
            count,
            value,
            into,
            counted,
        ],
    )?;
    assert!(matches!(
        ast.clauses.as_slice(),
        [
            LoopClause::Return(form),
            LoopClause::Accumulate {
                kind: AccumulatorKind::Nconc,
                form: nconc_form,
                variable: Some(nconc_variable),
            },
            LoopClause::Accumulate {
                kind: AccumulatorKind::Count,
                form: count_form,
                variable: Some(count_variable),
            },
        ] if *form == returned
            && *nconc_form == value
            && *nconc_variable == concatenated
            && *count_form == value
            && *count_variable == counted
    ));
    Ok(())
}

#[test]
fn expands_return_repeat_while_until_and_across_into_distinct_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let returned = symbol(&mut ctx, &runtime, "RETURNED")?;
    let predicate = symbol(&mut ctx, &runtime, "PREDICATE")?;
    let stop = symbol(&mut ctx, &runtime, "STOP")?;
    let item = symbol(&mut ctx, &runtime, "ITEM")?;
    let vector = symbol(&mut ctx, &runtime, "VECTOR")?;
    let expansion = expand_loop_ast(
        &mut ctx,
        &runtime,
        &LoopAst {
            name: None,
            clauses: vec![
                LoopClause::Repeat(Word::fixnum(2)),
                LoopClause::While(predicate),
                LoopClause::Until(stop),
                LoopClause::Across {
                    variable: item,
                    vector,
                },
                LoopClause::Return(returned),
            ],
        },
    )?;
    for operator_name in ["<=", "NOT", ">=", "ARRAY-TOTAL-SIZE", "AREF", "+"] {
        let operator = symbol(&mut ctx, &runtime, operator_name)?;
        assert!(
            contains_word(&mut ctx, expansion, operator)?,
            "missing {operator_name}"
        );
    }
    let return_from = symbol(&mut ctx, &runtime, "RETURN-FROM")?;
    assert!(contains_word(&mut ctx, expansion, return_from)?);
    assert!(contains_word(&mut ctx, expansion, returned)?);
    assert!(contains_word(&mut ctx, expansion, item)?);
    assert!(contains_word(&mut ctx, expansion, vector)?);
    Ok(())
}

#[test]
fn rejects_unimplemented_always_never_and_thereis_accumulators() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    for keyword in ["ALWAYS", "NEVER", "THEREIS"] {
        let keyword = symbol(&mut ctx, &runtime, keyword)?;
        assert_eq!(
            parse_loop(&mut ctx, &[keyword, value]),
            Err(ObjectError::TypeError),
            "unsupported accumulator keyword {keyword:?} should be rejected"
        );
    }
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
    let ast = parse_loop(&mut ctx, &input)?;
    assert!(matches!(ast.clauses[0], LoopClause::In { on: false, .. }));
    assert!(matches!(ast.clauses[1], LoopClause::In { on: true, .. }));
    assert!(matches!(ast.clauses[2], LoopClause::Across { .. }));
    assert_eq!(ast.clauses.len(), 3);
    Ok(())
}

#[test]
fn parses_hash_key_iteration_with_using_value() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let being = symbol(&mut ctx, &runtime, "BEING")?;
    let each = symbol(&mut ctx, &runtime, "EACH")?;
    let hash_key = symbol(&mut ctx, &runtime, "HASH-KEY")?;
    let hash_value = symbol(&mut ctx, &runtime, "HASH-VALUE")?;
    let of = symbol(&mut ctx, &runtime, "OF")?;
    let using_word = symbol(&mut ctx, &runtime, "USING")?;
    let using = list(&mut ctx, &runtime, &[hash_value, value])?;
    let ast = parse_loop(
        &mut ctx,
        &[
            for_word, key, being, each, hash_key, of, table, using_word, using,
        ],
    )?;
    assert!(matches!(
        ast.clauses.as_slice(),
        [LoopClause::Hash(HashClause {
            variable,
            kind: HashIterationKind::Key,
            table: parsed_table,
            using: Some((HashIterationKind::Value, parsed_value)),
        })] if *variable == key && *parsed_table == table && *parsed_value == value
    ));
    let the = symbol(&mut ctx, &runtime, "THE")?;
    let hash_values = symbol(&mut ctx, &runtime, "HASH-VALUES")?;
    let value_ast = parse_loop(
        &mut ctx,
        &[for_word, value, being, the, hash_values, of, table],
    )?;
    assert!(matches!(
        value_ast.clauses.as_slice(),
        [LoopClause::Hash(HashClause {
            kind: HashIterationKind::Value,
            using: None,
            ..
        })]
    ));
    Ok(())
}

#[test]
fn hash_iteration_rejects_malformed_and_same_kind_using_clauses() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let being = symbol(&mut ctx, &runtime, "BEING")?;
    let hash_key = symbol(&mut ctx, &runtime, "HASH-KEY")?;
    let hash_value = symbol(&mut ctx, &runtime, "HASH-VALUE")?;
    let of = symbol(&mut ctx, &runtime, "OF")?;
    let using_word = symbol(&mut ctx, &runtime, "USING")?;
    let using_key = list(&mut ctx, &runtime, &[hash_key, value])?;

    assert_eq!(
        parse_loop(&mut ctx, &[for_word, key, being, hash_key, table]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        parse_loop(
            &mut ctx,
            &[
                for_word, key, being, hash_key, of, table, using_word, using_key,
            ],
        ),
        Err(ObjectError::TypeError)
    );
    let using_value = list(&mut ctx, &runtime, &[hash_value, key])?;
    assert_eq!(
        parse_loop(
            &mut ctx,
            &[
                for_word,
                value,
                being,
                hash_value,
                of,
                table,
                using_word,
                using_value,
            ],
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn expands_hash_iteration_through_maphash() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let value = symbol(&mut ctx, &runtime, "VALUE")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let do_form = list(&mut ctx, &runtime, &[key, value])?;
    let expansion = expand_loop_ast(
        &mut ctx,
        &runtime,
        &LoopAst {
            name: None,
            clauses: vec![
                LoopClause::Hash(HashClause {
                    variable: key,
                    kind: HashIterationKind::Key,
                    table,
                    using: Some((HashIterationKind::Value, value)),
                }),
                LoopClause::Do(vec![do_form]),
            ],
        },
    )?;
    let let_form = elements(&mut ctx, expansion)?;
    let block = elements(&mut ctx, let_form[2])?;
    let progn = elements(&mut ctx, block[2])?;
    let maphash = elements(&mut ctx, progn[1])?;
    assert_eq!(maphash[0], symbol(&mut ctx, &runtime, "MAPHASH")?);
    assert_eq!(maphash[2], table);
    let lambda = elements(&mut ctx, maphash[1])?;
    assert_eq!(lambda[0], symbol(&mut ctx, &runtime, "LAMBDA")?);
    assert_eq!(elements(&mut ctx, lambda[1])?, vec![key, value]);
    assert_eq!(
        elements(&mut ctx, lambda[2])?[0],
        symbol(&mut ctx, &runtime, "TAGBODY")?
    );
    Ok(())
}

#[test]
fn expands_hash_iteration_without_using_variable() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let expansion = expand_loop_ast(
        &mut ctx,
        &runtime,
        &LoopAst {
            name: None,
            clauses: vec![
                LoopClause::Hash(HashClause {
                    variable: key,
                    kind: HashIterationKind::Key,
                    table,
                    using: None,
                }),
                LoopClause::Do(vec![key]),
            ],
        },
    )?;
    let let_form = elements(&mut ctx, expansion)?;
    let block = elements(&mut ctx, let_form[2])?;
    let progn = elements(&mut ctx, block[2])?;
    let maphash = elements(&mut ctx, progn[1])?;
    let lambda = elements(&mut ctx, maphash[1])?;
    let parameters = elements(&mut ctx, lambda[1])?;
    assert_eq!(parameters[0], key);
    assert_ne!(parameters[1], key);
    assert_eq!(maphash[2], table);
    Ok(())
}

#[test]
fn expands_for_directions_and_limits_to_matching_update_and_stop_operators()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let init = Word::fixnum(0);
    let limit = Word::fixnum(3);
    let clauses = [
        LoopClause::For(ForClause {
            variable: symbol(&mut ctx, &runtime, "UP")?,
            init,
            step: None,
            direction: None,
            limit: None,
        }),
        LoopClause::For(ForClause {
            variable: symbol(&mut ctx, &runtime, "DOWN")?,
            init,
            step: None,
            direction: Some(StepDirection::DownFrom),
            limit: None,
        }),
        LoopClause::For(ForClause {
            variable: symbol(&mut ctx, &runtime, "TO")?,
            init,
            step: Some(Word::fixnum(2)),
            direction: Some(StepDirection::From),
            limit: Some((LimitDirection::To, limit)),
        }),
        LoopClause::For(ForClause {
            variable: symbol(&mut ctx, &runtime, "UP-TO")?,
            init,
            step: None,
            direction: Some(StepDirection::UpFrom),
            limit: Some((LimitDirection::UpTo, limit)),
        }),
        LoopClause::For(ForClause {
            variable: symbol(&mut ctx, &runtime, "BELOW")?,
            init,
            step: None,
            direction: Some(StepDirection::From),
            limit: Some((LimitDirection::Below, limit)),
        }),
        LoopClause::For(ForClause {
            variable: symbol(&mut ctx, &runtime, "DOWN-TO")?,
            init,
            step: None,
            direction: Some(StepDirection::DownFrom),
            limit: Some((LimitDirection::DownTo, limit)),
        }),
        LoopClause::For(ForClause {
            variable: symbol(&mut ctx, &runtime, "ABOVE")?,
            init,
            step: None,
            direction: Some(StepDirection::From),
            limit: Some((LimitDirection::Above, limit)),
        }),
    ];
    let expansion = expand_loop_ast(
        &mut ctx,
        &runtime,
        &LoopAst {
            name: None,
            clauses: clauses.to_vec(),
        },
    )?;

    for operator_name in ["+", "-", ">", ">=", "<", "<="] {
        let operator = symbol(&mut ctx, &runtime, operator_name)?;
        assert!(
            contains_word(&mut ctx, expansion, operator)?,
            "missing FOR operator {operator_name}"
        );
    }
    for variable_name in ["UP", "DOWN", "TO", "UP-TO", "BELOW", "DOWN-TO", "ABOVE"] {
        let variable = symbol(&mut ctx, &runtime, variable_name)?;
        assert!(contains_word(&mut ctx, expansion, variable)?);
    }
    Ok(())
}

#[test]
fn expands_list_iteration_on_and_by_forms() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let item = symbol(&mut ctx, &runtime, "ITEM")?;
    let cursor = symbol(&mut ctx, &runtime, "CURSOR")?;
    let sequence = symbol(&mut ctx, &runtime, "SEQUENCE")?;
    let next = symbol(&mut ctx, &runtime, "NEXT")?;
    let expansion = expand_loop_ast(
        &mut ctx,
        &runtime,
        &LoopAst {
            name: None,
            clauses: vec![
                LoopClause::In {
                    variable: item,
                    sequence,
                    on: false,
                    by: None,
                },
                LoopClause::In {
                    variable: cursor,
                    sequence,
                    on: true,
                    by: Some(next),
                },
                LoopClause::Do(vec![item, cursor]),
            ],
        },
    )?;

    for operator_name in ["CAR", "CDR", "FUNCALL", "ENDP", "SETQ"] {
        let operator = symbol(&mut ctx, &runtime, operator_name)?;
        assert!(
            contains_word(&mut ctx, expansion, operator)?,
            "missing list iteration operator {operator_name}"
        );
    }
    assert!(contains_word(&mut ctx, expansion, next)?);
    assert!(contains_word(&mut ctx, expansion, sequence)?);
    Ok(())
}

#[test]
fn expands_hash_without_driver_with_stop_test_and_preserves_user_it_binding()
-> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let test = symbol(&mut ctx, &runtime, "TEST")?;
    let it = symbol(&mut ctx, &runtime, "IT")?;
    let expansion = expand_loop_ast(
        &mut ctx,
        &runtime,
        &LoopAst {
            name: None,
            clauses: vec![
                LoopClause::Hash(HashClause {
                    variable: key,
                    kind: HashIterationKind::Key,
                    table,
                    using: None,
                }),
                LoopClause::While(test),
                LoopClause::Conditional {
                    kind: ConditionalKind::When,
                    test,
                    then: vec![LoopClause::Do(vec![it])],
                    otherwise: vec![],
                },
            ],
        },
    )?;

    for operator_name in ["MAPHASH", "OR", "WHEN", "GO", "LET", "IF"] {
        let operator = symbol(&mut ctx, &runtime, operator_name)?;
        assert!(
            contains_word(&mut ctx, expansion, operator)?,
            "missing hash/conditional operator {operator_name}"
        );
    }
    assert!(contains_word(&mut ctx, expansion, it)?);
    assert!(contains_word(&mut ctx, expansion, table)?);
    Ok(())
}

#[test]
fn rejects_duplicate_hash_iteration_during_expansion() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let key = symbol(&mut ctx, &runtime, "KEY")?;
    let other_key = symbol(&mut ctx, &runtime, "OTHER-KEY")?;
    let table = symbol(&mut ctx, &runtime, "TABLE")?;
    let hash = |variable| {
        LoopClause::Hash(HashClause {
            variable,
            kind: HashIterationKind::Key,
            table,
            using: None,
        })
    };
    assert_eq!(
        expand_loop_ast(
            &mut ctx,
            &runtime,
            &LoopAst {
                name: None,
                clauses: vec![hash(key), hash(other_key)],
            },
        ),
        Err(ObjectError::TypeError)
    );

    let same_variable_using = LoopClause::Hash(HashClause {
        variable: key,
        kind: HashIterationKind::Key,
        table,
        using: Some((HashIterationKind::Value, key)),
    });
    assert_eq!(
        expand_loop_ast(
            &mut ctx,
            &runtime,
            &LoopAst {
                name: None,
                clauses: vec![same_variable_using],
            },
        ),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn parses_every_numeric_iteration_direction_and_limit() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let cases = [
        ("FROM", StepDirection::From, "TO", LimitDirection::To),
        (
            "UPFROM",
            StepDirection::UpFrom,
            "UPTO",
            LimitDirection::UpTo,
        ),
        (
            "DOWNFROM",
            StepDirection::DownFrom,
            "BELOW",
            LimitDirection::Below,
        ),
        (
            "DOWNFROM",
            StepDirection::DownFrom,
            "DOWNTO",
            LimitDirection::DownTo,
        ),
        (
            "UPFROM",
            StepDirection::UpFrom,
            "ABOVE",
            LimitDirection::Above,
        ),
    ];

    for (from_name, expected_direction, limit_name, expected_limit) in cases {
        let from = symbol(&mut ctx, &runtime, from_name)?;
        let limit = symbol(&mut ctx, &runtime, limit_name)?;
        let by = symbol(&mut ctx, &runtime, "BY")?;
        let ast = parse_loop(
            &mut ctx,
            &[
                for_word,
                x,
                from,
                Word::fixnum(10),
                by,
                Word::fixnum(2),
                limit,
                Word::fixnum(20),
            ],
        )?;
        assert!(matches!(
            ast.clauses.as_slice(),
            [LoopClause::For(ForClause {
                variable,
                init,
                step: Some(step),
                direction: Some(direction),
                limit: Some((limit_direction, limit_value)),
            })]
            if *variable == x
                && *init == Word::fixnum(10)
                && *step == Word::fixnum(2)
                && *direction == expected_direction
                && *limit_direction == expected_limit
                && *limit_value == Word::fixnum(20)
        ));
    }

    let by = symbol(&mut ctx, &runtime, "BY")?;
    let default_ast = parse_loop(&mut ctx, &[for_word, x, by, Word::fixnum(3)])?;
    assert!(matches!(
        default_ast.clauses.as_slice(),
        [LoopClause::For(ForClause {
            init,
            step: Some(step),
            direction: None,
            limit: None,
            ..
        })] if *init == Word::fixnum(0) && *step == Word::fixnum(3)
    ));
    Ok(())
}

#[test]
fn numeric_iteration_expansion_uses_direction_and_limit_operators() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let downfrom = symbol(&mut ctx, &runtime, "DOWNFROM")?;
    let by = symbol(&mut ctx, &runtime, "BY")?;
    let above = symbol(&mut ctx, &runtime, "ABOVE")?;
    let ast = parse_loop(
        &mut ctx,
        &[
            for_word,
            x,
            downfrom,
            Word::fixnum(10),
            by,
            Word::fixnum(2),
            above,
            Word::fixnum(3),
        ],
    )?;
    let expansion = expand_loop_ast(&mut ctx, &runtime, &ast)?;
    let outer = elements(&mut ctx, expansion)?;
    let block = elements(&mut ctx, outer[2])?;
    let progn = elements(&mut ctx, block[2])?;
    let tagbody = elements(&mut ctx, progn[1])?;
    let stop = elements(&mut ctx, tagbody[2])?;
    assert_eq!(stop[0], symbol(&mut ctx, &runtime, "WHEN")?);
    let disjunction = elements(&mut ctx, stop[1])?;
    let comparison = elements(&mut ctx, disjunction[1])?;
    assert_eq!(comparison[0], symbol(&mut ctx, &runtime, "<=")?);
    let go_end = elements(&mut ctx, stop[2])?;
    assert_eq!(go_end[0], symbol(&mut ctx, &runtime, "GO")?);
    let setq = elements(&mut ctx, tagbody[3])?;
    let update = elements(&mut ctx, setq[2])?;
    assert_eq!(update[0], symbol(&mut ctx, &runtime, "-")?);
    Ok(())
}

#[test]
fn parses_all_top_level_clause_families_and_conditional_branches() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let named = symbol(&mut ctx, &runtime, "NAMED")?;
    let name = symbol(&mut ctx, &runtime, "DONE")?;
    let with = symbol(&mut ctx, &runtime, "WITH")?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let equals = symbol(&mut ctx, &runtime, "=")?;
    let initially = symbol(&mut ctx, &runtime, "INITIALLY")?;
    let finally = symbol(&mut ctx, &runtime, "FINALLY")?;
    let do_word = symbol(&mut ctx, &runtime, "DO")?;
    let return_word = symbol(&mut ctx, &runtime, "RETURN")?;
    let when = symbol(&mut ctx, &runtime, "WHEN")?;
    let and = symbol(&mut ctx, &runtime, "AND")?;
    let else_word = symbol(&mut ctx, &runtime, "ELSE")?;
    let end = symbol(&mut ctx, &runtime, "END")?;
    let collect = symbol(&mut ctx, &runtime, "COLLECT")?;
    let into = symbol(&mut ctx, &runtime, "INTO")?;
    let result = symbol(&mut ctx, &runtime, "RESULT")?;
    let first = symbol(&mut ctx, &runtime, "FIRST")?;
    let second = symbol(&mut ctx, &runtime, "SECOND")?;
    let third = symbol(&mut ctx, &runtime, "THIRD")?;
    let ast = parse_loop(
        &mut ctx,
        &[
            named,
            name,
            with,
            x,
            equals,
            Word::fixnum(1),
            initially,
            first,
            finally,
            second,
            when,
            x,
            do_word,
            third,
            and,
            collect,
            x,
            into,
            result,
            else_word,
            return_word,
            x,
            end,
        ],
    )?;
    assert_eq!(ast.name, Some(name));
    assert!(matches!(
        ast.clauses.as_slice(),
        [
            LoopClause::With { variable, init },
            LoopClause::Initially(initially_forms),
            LoopClause::Finally(finally_forms),
            LoopClause::Conditional {
                kind: ConditionalKind::When,
                test,
                then,
                otherwise,
            },
        ] if *variable == x
            && *init == Word::fixnum(1)
            && initially_forms == &vec![first]
            && finally_forms == &vec![second]
            && *test == x
            && matches!(then.as_slice(), [LoopClause::Do(forms), LoopClause::Accumulate { kind: AccumulatorKind::Collect, form, variable: Some(target) }] if forms == &vec![third] && *form == x && *target == result)
            && matches!(otherwise.as_slice(), [LoopClause::Return(value)] if *value == x)
    ));
    Ok(())
}

#[test]
fn parses_alias_control_and_accumulator_clauses() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let y = symbol(&mut ctx, &runtime, "Y")?;
    let input = [
        symbol(&mut ctx, &runtime, "AS")?,
        y,
        symbol(&mut ctx, &runtime, "FROM")?,
        Word::fixnum(1),
        symbol(&mut ctx, &runtime, "REPEAT")?,
        Word::fixnum(2),
        symbol(&mut ctx, &runtime, "WHILE")?,
        symbol(&mut ctx, &runtime, "READY")?,
        symbol(&mut ctx, &runtime, "UNTIL")?,
        symbol(&mut ctx, &runtime, "DONE")?,
        symbol(&mut ctx, &runtime, "UNLESS")?,
        symbol(&mut ctx, &runtime, "SKIP")?,
        symbol(&mut ctx, &runtime, "DO")?,
        symbol(&mut ctx, &runtime, "ACTION")?,
        symbol(&mut ctx, &runtime, "IF")?,
        symbol(&mut ctx, &runtime, "KEEP")?,
        symbol(&mut ctx, &runtime, "RETURN")?,
        y,
        symbol(&mut ctx, &runtime, "APPEND")?,
        y,
        symbol(&mut ctx, &runtime, "NCONC")?,
        y,
        symbol(&mut ctx, &runtime, "COUNT")?,
        y,
        symbol(&mut ctx, &runtime, "SUM")?,
        y,
        symbol(&mut ctx, &runtime, "MAXIMIZE")?,
        y,
        symbol(&mut ctx, &runtime, "MINIMIZE")?,
        y,
    ];
    let ast = parse_loop(&mut ctx, &input)?;
    assert_eq!(ast.clauses.len(), 12);
    assert!(matches!(ast.clauses[0], LoopClause::For(ForClause { variable, .. }) if variable == y));
    assert!(matches!(ast.clauses[1], LoopClause::Repeat(count) if count == Word::fixnum(2)));
    assert!(matches!(ast.clauses[2], LoopClause::While(test) if test == input[7]));
    assert!(matches!(ast.clauses[3], LoopClause::Until(test) if test == input[9]));
    assert!(matches!(
        ast.clauses[4],
        LoopClause::Conditional {
            kind: ConditionalKind::Unless,
            ..
        }
    ));
    assert!(matches!(
        ast.clauses[5],
        LoopClause::Conditional {
            kind: ConditionalKind::If,
            ..
        }
    ));
    for (clause, kind) in ast.clauses[6..].iter().zip([
        AccumulatorKind::Append,
        AccumulatorKind::Nconc,
        AccumulatorKind::Count,
        AccumulatorKind::Sum,
        AccumulatorKind::Maximize,
        AccumulatorKind::Minimize,
    ]) {
        assert!(
            matches!(clause, LoopClause::Accumulate { kind: actual, form, variable: None } if *actual == kind && *form == y)
        );
    }
    Ok(())
}

#[test]
fn malformed_loop_clauses_report_type_errors() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let by = symbol(&mut ctx, &runtime, "BY")?;
    let then = symbol(&mut ctx, &runtime, "THEN")?;
    let do_word = symbol(&mut ctx, &runtime, "DO")?;
    let unknown = symbol(&mut ctx, &runtime, "NOT-A-CLAUSE")?;
    let when = symbol(&mut ctx, &runtime, "WHEN")?;
    let and = symbol(&mut ctx, &runtime, "AND")?;
    let equals = symbol(&mut ctx, &runtime, "=")?;

    let malformed = [
        vec![for_word],
        vec![for_word, x, by],
        vec![for_word, x, by, Word::fixnum(1), by, Word::fixnum(2)],
        vec![for_word, x, then, Word::fixnum(1), then, Word::fixnum(2)],
        vec![
            for_word,
            x,
            equals,
            Word::fixnum(1),
            then,
            Word::fixnum(2),
            by,
            Word::fixnum(3),
        ],
        vec![unknown],
        vec![when, Word::fixnum(1), and],
        vec![when, Word::fixnum(1), unknown],
        vec![do_word, when],
    ];
    for input in malformed {
        assert_eq!(parse_loop(&mut ctx, &input), Err(ObjectError::TypeError));
    }
    Ok(())
}
