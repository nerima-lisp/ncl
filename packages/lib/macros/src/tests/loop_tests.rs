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
    let ast = parse_loop(&mut ctx, &runtime, &input)?;
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
fn parses_and_expands_thereis_once() -> Result<(), ObjectError> {
    fn count_form(ctx: &ThreadContext, tree: Word, form: Word) -> Result<usize, ObjectError> {
        if tree == form {
            return Ok(1);
        }
        if !tree.is_cons() {
            return Ok(0);
        }
        Ok(count_form(ctx, ncl_object::car(ctx, tree)?, form)?
            + count_form(ctx, ncl_object::cdr(ctx, tree)?, form)?)
    }

    let (runtime, mut ctx) = fixture()?;
    let x = symbol(&mut ctx, &runtime, "X")?;
    let plus = symbol(&mut ctx, &runtime, "+")?;
    let form = list(&mut ctx, &runtime, &[plus, x, Word::fixnum(1)])?;
    let thereis = symbol(&mut ctx, &runtime, "THEREIS")?;
    let ast = parse_loop(&mut ctx, &runtime, &[thereis, form])?;
    assert!(matches!(ast.clauses.as_slice(), [LoopClause::Thereis(value)] if *value == form));

    let expansion = expand_loop_ast(&mut ctx, &runtime, &ast)?;
    assert_eq!(count_form(&ctx, expansion, form)?, 1);
    Ok(())
}

#[test]
fn parses_always_and_never_termination_clauses() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let form = symbol(&mut ctx, &runtime, "P")?;
    let always = symbol(&mut ctx, &runtime, "ALWAYS")?;
    let never = symbol(&mut ctx, &runtime, "NEVER")?;
    let ast = parse_loop(&mut ctx, &runtime, &[always, form, never, form])?;
    assert!(matches!(
        ast.clauses.as_slice(),
        [LoopClause::Always(first), LoopClause::Never(second)] if *first == form && *second == form
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
        &runtime,
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
    let ast = parse_loop(&mut ctx, &runtime, &input)?;
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
        &runtime,
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
        &runtime,
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
        parse_loop(&mut ctx, &runtime, &[for_word, key, being, hash_key, table]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        parse_loop(
            &mut ctx,
            &runtime,
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
            &runtime,
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

struct PackageIterationCase {
    case_id: &'static str,
    article_name: &'static str,
    kind_name: &'static str,
    package_preposition: Option<&'static str>,
    expected_kind: PackageIterationKind,
    type_name: Option<&'static str>,
    following_for: bool,
}

fn assert_package_iteration_case(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    case: &PackageIterationCase,
) -> Result<(), ObjectError> {
    let for_word = symbol(ctx, runtime, "FOR")?;
    let variable = symbol(ctx, runtime, "SYMBOL")?;
    let status = symbol(ctx, runtime, "STATUS")?;
    let being = symbol(ctx, runtime, "BEING")?;
    let package = symbol(ctx, runtime, "PACKAGE")?;
    let equals = symbol(ctx, runtime, "=")?;
    let status_form = symbol(ctx, runtime, "STATUS-FORM")?;

    let mut input = vec![for_word, variable];
    let expected_type_spec = if let Some(type_name) = case.type_name {
        let type_spec = symbol(ctx, runtime, type_name)?;
        input.push(type_spec);
        Some(type_spec)
    } else {
        None
    };
    input.extend([
        being,
        symbol(ctx, runtime, case.article_name)?,
        symbol(ctx, runtime, case.kind_name)?,
    ]);
    let expected_package = if let Some(preposition) = case.package_preposition {
        input.push(symbol(ctx, runtime, preposition)?);
        input.push(package);
        Some(package)
    } else {
        None
    };
    if case.following_for {
        input.extend([for_word, status, equals, status_form]);
    }

    let ast = parse_loop(ctx, runtime, &input)?;
    let LoopClause::Package(parsed) = &ast.clauses[0] else {
        panic!("{}: expected package iteration clause", case.case_id);
    };
    assert_eq!(parsed.kind, case.expected_kind, "{}", case.case_id);
    assert_eq!(parsed.package, expected_package, "{}", case.case_id);
    assert_eq!(parsed.type_spec, expected_type_spec, "{}", case.case_id);
    if case.following_for {
        assert!(matches!(
            ast.clauses.as_slice(),
            [
                LoopClause::Package(_),
                LoopClause::EqualsThen {
                    variable: parsed_status,
                    then: None,
                    ..
                }
            ] if *parsed_status == status
        ));
    } else {
        assert_eq!(ast.clauses.len(), 1, "{}", case.case_id);
    }
    Ok(())
}

#[test]
fn parses_symbol_package_iteration_aliases_and_following_for_binding() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let cases = [
        PackageIterationCase {
            case_id: "each-symbol-of",
            article_name: "EACH",
            kind_name: "SYMBOL",
            package_preposition: Some("OF"),
            expected_kind: PackageIterationKind::Symbols,
            type_name: None,
            following_for: true,
        },
        PackageIterationCase {
            case_id: "the-symbols-in",
            article_name: "THE",
            kind_name: "SYMBOLS",
            package_preposition: Some("IN"),
            expected_kind: PackageIterationKind::Symbols,
            type_name: None,
            following_for: false,
        },
        PackageIterationCase {
            case_id: "each-present-symbol-in",
            article_name: "EACH",
            kind_name: "PRESENT-SYMBOL",
            package_preposition: Some("IN"),
            expected_kind: PackageIterationKind::PresentSymbols,
            type_name: None,
            following_for: false,
        },
        PackageIterationCase {
            case_id: "the-present-symbols-default-package",
            article_name: "THE",
            kind_name: "PRESENT-SYMBOLS",
            package_preposition: None,
            expected_kind: PackageIterationKind::PresentSymbols,
            type_name: Some("INTEGER"),
            following_for: false,
        },
        PackageIterationCase {
            case_id: "each-external-symbol-in",
            article_name: "EACH",
            kind_name: "EXTERNAL-SYMBOL",
            package_preposition: Some("IN"),
            expected_kind: PackageIterationKind::ExternalSymbols,
            type_name: None,
            following_for: false,
        },
        PackageIterationCase {
            case_id: "the-external-symbols-of",
            article_name: "THE",
            kind_name: "EXTERNAL-SYMBOLS",
            package_preposition: Some("OF"),
            expected_kind: PackageIterationKind::ExternalSymbols,
            type_name: None,
            following_for: false,
        },
    ];
    for case in cases {
        assert_package_iteration_case(&mut ctx, &runtime, &case)?;
    }
    Ok(())
}

#[test]
fn unsupported_package_iteration_reports_the_clause() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let input = [
        symbol(&mut ctx, &runtime, "FOR")?,
        symbol(&mut ctx, &runtime, "SYMBOL")?,
        symbol(&mut ctx, &runtime, "BEING")?,
        symbol(&mut ctx, &runtime, "THE")?,
        symbol(&mut ctx, &runtime, "WIDGETS")?,
    ];
    assert_eq!(
        parse_loop(&mut ctx, &runtime, &input),
        Err(ObjectError::TypeError)
    );
    let Some(ncl_object::LispError::SimpleError { format_control, .. }) =
        ctx.take_pending_lisp_error()
    else {
        panic!("unsupported clause should set a simple error");
    };
    let message = (0..ncl_object::string_length(&ctx, format_control)?)
        .map(|index| ncl_object::string_ref(&ctx, format_control, index))
        .collect::<std::result::Result<String, _>>()?;
    assert!(message.contains("BEING THE WIDGETS"));
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

struct PackageExpansionCase {
    case_id: &'static str,
    kind: PackageIterationKind,
    mode_name: &'static str,
    package_name: Option<&'static str>,
    type_name: Option<&'static str>,
}

fn assert_package_expansion_case(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    case: &PackageExpansionCase,
) -> Result<(), ObjectError> {
    let variable = symbol(ctx, runtime, "SYMBOL")?;
    let package = case
        .package_name
        .map(|name| symbol(ctx, runtime, name))
        .transpose()?;
    let type_spec = case
        .type_name
        .map(|name| symbol(ctx, runtime, name))
        .transpose()?;
    let ast = LoopAst {
        name: None,
        clauses: vec![LoopClause::Package(PackageClause {
            variable,
            type_spec,
            kind: case.kind,
            package,
        })],
    };
    let expansion = expand_loop_ast(ctx, runtime, &ast)?;
    let outer = elements(ctx, expansion)?;
    assert_eq!(
        outer[0],
        symbol(ctx, runtime, "LET")?,
        "{}: outer form",
        case.case_id
    );

    let bindings = elements(ctx, outer[1])?;
    assert_eq!(bindings.len(), 2, "{}: package bindings", case.case_id);
    let cursor_binding = elements(ctx, bindings[0])?;
    let sequence = elements(ctx, cursor_binding[1])?;
    let iterator = symbol(ctx, runtime, "NCL-EXT::%PACKAGE-ITERATION-SYMBOLS")?;
    let mode = symbol(ctx, runtime, case.mode_name)?;
    let package = match package {
        Some(package) => package,
        None => symbol(ctx, runtime, "COMMON-LISP::*PACKAGE*")?,
    };
    assert_eq!(
        sequence,
        vec![iterator, mode, package],
        "{}: iterator call",
        case.case_id
    );

    let block_index = if let Some(type_spec) = type_spec {
        let declaration = elements(ctx, outer[2])?;
        let declare = symbol(ctx, runtime, "DECLARE")?;
        let type_operator = symbol(ctx, runtime, "TYPE")?;
        assert_eq!(declaration.len(), 2, "{}: declaration form", case.case_id);
        assert_eq!(
            declaration[0], declare,
            "{}: declaration operator",
            case.case_id
        );
        assert_eq!(
            elements(ctx, declaration[1])?,
            vec![type_operator, type_spec, variable],
            "{}: type declaration",
            case.case_id
        );
        3
    } else {
        2
    };
    let block = elements(ctx, outer[block_index])?;
    assert_eq!(
        block[0],
        symbol(ctx, runtime, "BLOCK")?,
        "{}: loop block",
        case.case_id
    );
    Ok(())
}

#[test]
fn package_iteration_expansion_covers_modes_packages_and_declarations() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let cases = [
        PackageExpansionCase {
            case_id: "symbols-explicit-package",
            kind: PackageIterationKind::Symbols,
            mode_name: "KEYWORD::SYMBOLS",
            package_name: Some("EXPANSION-PACKAGE"),
            type_name: None,
        },
        PackageExpansionCase {
            case_id: "present-symbols-default-package",
            kind: PackageIterationKind::PresentSymbols,
            mode_name: "KEYWORD::PRESENT",
            package_name: None,
            type_name: Some("INTEGER"),
        },
        PackageExpansionCase {
            case_id: "external-symbols-explicit-package",
            kind: PackageIterationKind::ExternalSymbols,
            mode_name: "KEYWORD::EXTERNAL",
            package_name: Some("EXPANSION-PACKAGE"),
            type_name: Some("INTEGER"),
        },
    ];
    for case in cases {
        assert_package_expansion_case(&mut ctx, &runtime, &case)?;
    }
    Ok(())
}

#[test]
fn equals_then_expansion_uses_then_or_initial_value_for_updates() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let package = symbol(&mut ctx, &runtime, "EXPANSION-PACKAGE")?;
    let symbol_variable = symbol(&mut ctx, &runtime, "SYMBOL")?;
    let status = symbol(&mut ctx, &runtime, "STATUS")?;
    let init = Word::fixnum(17);
    let step = Word::fixnum(29);
    let cases = [
        ("then-omitted", None, init),
        ("then-present", Some(step), step),
    ];

    for (case_id, then, expected_update) in cases {
        let expansion = expand_loop_ast(
            &mut ctx,
            &runtime,
            &LoopAst {
                name: None,
                clauses: vec![
                    LoopClause::Package(PackageClause {
                        variable: symbol_variable,
                        type_spec: None,
                        kind: PackageIterationKind::Symbols,
                        package: Some(package),
                    }),
                    LoopClause::EqualsThen {
                        variable: status,
                        init,
                        then,
                    },
                ],
            },
        )?;
        let outer = elements(&mut ctx, expansion)?;
        let bindings = elements(&mut ctx, outer[1])?;
        assert_eq!(bindings.len(), 3, "{case_id}: generated bindings");
        let cursor = elements(&mut ctx, bindings[0])?[0];
        let block = elements(&mut ctx, outer[2])?;
        let progn = elements(&mut ctx, block[2])?;
        let setq = symbol(&mut ctx, &runtime, "SETQ")?;
        let car = symbol(&mut ctx, &runtime, "CAR")?;
        let cdr = symbol(&mut ctx, &runtime, "CDR")?;

        let package_initial = elements(&mut ctx, progn[1])?;
        assert_eq!(
            &package_initial[..2],
            &[setq, symbol_variable],
            "{case_id}: package initial assignment"
        );
        assert_eq!(
            elements(&mut ctx, package_initial[2])?,
            vec![car, cursor],
            "{case_id}: package initial value"
        );
        assert_eq!(
            elements(&mut ctx, progn[2])?,
            vec![setq, status, init],
            "{case_id}: equals initial value"
        );
        let tagbody = elements(&mut ctx, progn[3])?;
        let updates = elements(&mut ctx, tagbody[3])?;
        assert_eq!(updates.len(), 7, "{case_id}: combined update form");
        assert_eq!(
            &updates[..2],
            &[setq, cursor],
            "{case_id}: package cursor update"
        );
        assert_eq!(
            elements(&mut ctx, updates[2])?,
            vec![cdr, cursor],
            "{case_id}: next package cursor"
        );
        assert_eq!(
            updates[3], symbol_variable,
            "{case_id}: iteration variable update"
        );
        assert_eq!(
            elements(&mut ctx, updates[4])?,
            vec![car, cursor],
            "{case_id}: next package symbol"
        );
        assert_eq!(updates[5], status, "{case_id}: equals variable update");
        assert_eq!(
            updates[6], expected_update,
            "{case_id}: equals update value"
        );
    }
    Ok(())
}

#[test]
fn package_iteration_parser_rejects_malformed_and_unknown_clauses() -> Result<(), ObjectError> {
    let (runtime, mut ctx) = fixture()?;
    let for_word = symbol(&mut ctx, &runtime, "FOR")?;
    let variable = symbol(&mut ctx, &runtime, "SYMBOL")?;
    let being = symbol(&mut ctx, &runtime, "BEING")?;
    let the = symbol(&mut ctx, &runtime, "THE")?;
    let symbols = symbol(&mut ctx, &runtime, "SYMBOLS")?;
    let in_word = symbol(&mut ctx, &runtime, "IN")?;
    let of_word = symbol(&mut ctx, &runtime, "OF")?;
    let package = symbol(&mut ctx, &runtime, "PACKAGE")?;

    let malformed = [
        ("missing article", vec![for_word, variable, being, symbols]),
        ("missing kind", vec![for_word, variable, being, the]),
        (
            "missing IN package",
            vec![for_word, variable, being, the, symbols, in_word],
        ),
        (
            "missing OF package",
            vec![for_word, variable, being, the, symbols, of_word],
        ),
    ];
    for (case_id, input) in malformed {
        assert_eq!(
            parse_loop(&mut ctx, &runtime, &input),
            Err(ObjectError::TypeError),
            "{case_id}"
        );
        assert!(
            ctx.take_pending_lisp_error().is_none(),
            "{case_id}: malformed input has no unsupported-clause diagnostic"
        );
    }

    let widgets = symbol(&mut ctx, &runtime, "WIDGETS")?;
    let continuation = [
        for_word, variable, being, the, symbols, in_word, package, widgets,
    ];
    assert_eq!(
        parse_loop(&mut ctx, &runtime, &continuation),
        Err(ObjectError::TypeError)
    );
    let Some(ncl_object::LispError::SimpleError { format_control, .. }) =
        ctx.take_pending_lisp_error()
    else {
        panic!("unknown continuation should set a simple error");
    };
    let message = (0..ncl_object::string_length(&ctx, format_control)?)
        .map(|index| ncl_object::string_ref(&ctx, format_control, index))
        .collect::<std::result::Result<String, _>>()?;
    assert_eq!(message, "Unsupported LOOP clause: WIDGETS");
    Ok(())
}
