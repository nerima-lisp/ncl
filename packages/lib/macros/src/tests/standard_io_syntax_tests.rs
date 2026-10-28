use super::*;

fn standard_io_symbol_text(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    let name = ncl_object::symbol_name(ctx, symbol)?;
    (0..ncl_object::string_length(ctx, name)?)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect()
}

fn standard_io_string_text(ctx: &ThreadContext, string: Word) -> Result<String, ObjectError> {
    (0..ncl_object::string_length(ctx, string)?)
        .map(|index| ncl_object::string_ref(ctx, string, index))
        .collect()
}

fn standard_io_binding(
    ctx: &mut ThreadContext,
    bindings: Word,
    name: &str,
) -> Result<Word, ObjectError> {
    for binding in elements(ctx, bindings)? {
        let pair = elements(ctx, binding)?;
        if pair.len() == 2 && standard_io_symbol_text(ctx, pair[0])? == name {
            return Ok(pair[1]);
        }
    }
    Err(ObjectError::TypeError)
}

fn standard_io_call_arguments(
    ctx: &mut ThreadContext,
    call: Word,
    operator_name: &str,
) -> Result<Vec<Word>, ObjectError> {
    let call = elements(ctx, call)?;
    if call.len() != 2 || standard_io_symbol_text(ctx, call[0])? != operator_name {
        return Err(ObjectError::TypeError);
    }
    Ok(call.iter().copied().skip(1).collect())
}

#[test]
#[allow(clippy::too_many_lines)]
fn standard_io_syntax_expansion_binds_standard_values() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    register(&runtime)?;
    ctx.register(&runtime)?;

    let mut body = symbol(&mut ctx, &runtime, "BODY")?;
    let mut root_tokens = vec![ncl_object::push_root(&mut ctx, &mut body)];
    let operator = symbol(&mut ctx, &runtime, "WITH-STANDARD-IO-SYNTAX")?;
    let mut input = list(&mut ctx, &runtime, &[operator, body])?;
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut input));

    ctx.set_gc_stress(true);
    let mut values = ncl_object::MultipleValues::new();
    let input_args = [input];
    let args = ncl_object::BuiltinArgs::new(&input_args);
    let expand = callback_for("WITH-STANDARD-IO-SYNTAX").ok_or(ObjectError::UndefinedFunction)?;
    let mut expansion = expand(&mut ctx, &runtime, &args, &mut values)?;
    root_tokens.push(ncl_object::push_root(&mut ctx, &mut expansion));
    ctx.collect(true)?;

    let result = (|| {
        let form = elements(&mut ctx, expansion)?;
        if form.len() != 3 || standard_io_symbol_text(&ctx, form[0])? != "LET" {
            return Err(ObjectError::TypeError);
        }
        // check-added-lines: allow(panic) test-only assertion
        assert_eq!(form[2], body, "the macro body remains inside the LET");

        let bindings = elements(&mut ctx, form[1])?;
        let binding_names = bindings
            .iter()
            .map(|binding| {
                let pair = elements(&mut ctx, *binding)?;
                if pair.len() != 2 {
                    return Err(ObjectError::TypeError);
                }
                standard_io_symbol_text(&ctx, pair[0])
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expected_names = [
            "*PACKAGE*",
            "*PRINT-ARRAY*",
            "*PRINT-BASE*",
            "*PRINT-CASE*",
            "*PRINT-CIRCLE*",
            "*PRINT-ESCAPE*",
            "*PRINT-GENSYM*",
            "*PRINT-LENGTH*",
            "*PRINT-LEVEL*",
            "*PRINT-LINES*",
            "*PRINT-MISER-WIDTH*",
            "*PRINT-PPRINT-DISPATCH*",
            "*PRINT-PRETTY*",
            "*PRINT-RADIX*",
            "*PRINT-READABLY*",
            "*PRINT-RIGHT-MARGIN*",
            "*READ-BASE*",
            "*READ-DEFAULT-FLOAT-FORMAT*",
            "*READ-EVAL*",
            "*READ-SUPPRESS*",
            "*READTABLE*",
            "*PRINT-CIRCLE-NOT-SHARED*",
            "*PRINT-VECTOR-LENGTH*",
        ];
        // check-added-lines: allow(panic) test-only assertion
        assert_eq!(
            binding_names,
            expected_names.map(str::to_owned),
            "the standard and implementation-specific controls are each bound once"
        );

        for name in [
            "*PRINT-ARRAY*",
            "*PRINT-ESCAPE*",
            "*PRINT-GENSYM*",
            "*PRINT-READABLY*",
            "*READ-EVAL*",
        ] {
            let value = standard_io_binding(&mut ctx, form[1], name)?;
            // check-added-lines: allow(panic) test-only assertion
            assert_eq!(standard_io_symbol_text(&ctx, value)?, "T", "{name}");
        }

        for name in [
            "*PRINT-CIRCLE*",
            "*PRINT-LENGTH*",
            "*PRINT-LEVEL*",
            "*PRINT-LINES*",
            "*PRINT-MISER-WIDTH*",
            "*PRINT-PRETTY*",
            "*PRINT-RADIX*",
            "*PRINT-RIGHT-MARGIN*",
            "*READ-SUPPRESS*",
            "*PRINT-CIRCLE-NOT-SHARED*",
            "*PRINT-VECTOR-LENGTH*",
        ] {
            // check-added-lines: allow(panic) test-only assertion
            assert_eq!(
                standard_io_binding(&mut ctx, form[1], name)?,
                Word::NIL,
                "{name}"
            );
        }

        for name in ["*PRINT-BASE*", "*READ-BASE*"] {
            // check-added-lines: allow(panic) test-only assertion
            assert_eq!(
                standard_io_binding(&mut ctx, form[1], name)?.as_fixnum(),
                Some(10),
                "{name}"
            );
        }

        let print_case = standard_io_binding(&mut ctx, form[1], "*PRINT-CASE*")?;
        // check-added-lines: allow(panic) test-only assertion
        assert_eq!(standard_io_symbol_text(&ctx, print_case)?, "UPCASE");
        let float_format = standard_io_binding(&mut ctx, form[1], "*READ-DEFAULT-FLOAT-FORMAT*")?;
        let float_format = elements(&mut ctx, float_format)?;
        if float_format.len() != 2 || standard_io_symbol_text(&ctx, float_format[0])? != "QUOTE" {
            return Err(ObjectError::TypeError);
        }
        // check-added-lines: allow(panic) test-only assertion
        assert_eq!(
            standard_io_symbol_text(&ctx, float_format[1])?,
            "SINGLE-FLOAT"
        );

        let package = standard_io_binding(&mut ctx, form[1], "*PACKAGE*")?;
        let package_args = standard_io_call_arguments(&mut ctx, package, "FIND-PACKAGE")?;
        if package_args.len() != 1 {
            return Err(ObjectError::TypeError);
        }
        // check-added-lines: allow(panic) test-only assertion
        assert_eq!(standard_io_string_text(&ctx, package_args[0])?, "CL-USER");

        for (name, operator) in [
            ("*PRINT-PPRINT-DISPATCH*", "COPY-PPRINT-DISPATCH"),
            ("*READTABLE*", "COPY-READTABLE"),
        ] {
            let value = standard_io_binding(&mut ctx, form[1], name)?;
            let arguments = standard_io_call_arguments(&mut ctx, value, operator)?;
            // check-added-lines: allow(panic) test-only assertion
            assert_eq!(arguments, [Word::NIL], "{name}");
        }
        Ok(())
    })();

    for token in root_tokens.into_iter().rev() {
        // check-added-lines: allow(panic) test-only assertion
        assert!(ncl_object::pop_root(&mut ctx, token));
    }
    result
}

fn standard_io_fill_until_capacity(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    roots: &mut [Word],
    start: usize,
    words: usize,
) -> Result<usize, ObjectError> {
    let mut next_root = start;
    loop {
        if next_root == roots.len() {
            return Err(ObjectError::Layout);
        }
        match ncl_object::allocate(ctx, runtime, 0x7f, words) {
            Ok(value) => {
                roots[next_root] = value;
                next_root += 1;
            }
            Err(error) => {
                if format!("{error:?}") != "Storage(CapacityExceeded)" {
                    return Err(error);
                }
                return Ok(next_root);
            }
        }
    }
}

fn standard_io_reserve_heap_cells(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    roots: &mut [Word],
    initial_root: usize,
    released_cells: usize,
) -> Result<(), ObjectError> {
    let mut next_root = initial_root;
    let mut reserve_root = None;
    for words in [65_536, 4_096, 256] {
        let stage_start = next_root;
        next_root = standard_io_fill_until_capacity(ctx, runtime, roots, next_root, words)?;
        if words == 4_096 {
            if next_root == stage_start {
                return Err(ObjectError::Layout);
            }
            reserve_root = Some(next_root - 1);
        }
    }

    let reserve_root = reserve_root.ok_or(ObjectError::Layout)?;
    roots[reserve_root] = Word::NIL;
    ctx.collect(true)?;

    let single_cell_start = next_root;
    next_root = standard_io_fill_until_capacity(ctx, runtime, roots, next_root, 0)?;
    if next_root - single_cell_start < released_cells {
        return Err(ObjectError::Layout);
    }

    for root in &mut roots[next_root - released_cells..next_root] {
        *root = Word::NIL;
    }
    ctx.collect(true)
}

fn standard_io_expansion_with_released_cells(
    released_cells: usize,
    preinterned_symbols: usize,
) -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    register(&runtime)?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let operator = symbol(&mut ctx, &runtime, "WITH-STANDARD-IO-SYNTAX")?;
    let body = symbol(&mut ctx, &runtime, "BODY")?;
    let input = list(&mut ctx, &runtime, &[operator, body])?;
    let mut names = vec![
        "T",
        "KEYWORD::UPCASE",
        "SINGLE-FLOAT",
        "FIND-PACKAGE",
        "COPY-PPRINT-DISPATCH",
        "COPY-READTABLE",
        "QUOTE",
    ];
    names.extend([
        "*PACKAGE*",
        "*PRINT-ARRAY*",
        "*PRINT-BASE*",
        "*PRINT-CASE*",
        "*PRINT-CIRCLE*",
        "*PRINT-ESCAPE*",
        "*PRINT-GENSYM*",
        "*PRINT-LENGTH*",
        "*PRINT-LEVEL*",
        "*PRINT-LINES*",
        "*PRINT-MISER-WIDTH*",
        "*PRINT-PPRINT-DISPATCH*",
        "*PRINT-PRETTY*",
        "*PRINT-RADIX*",
        "*PRINT-READABLY*",
        "*PRINT-RIGHT-MARGIN*",
        "*READ-BASE*",
        "*READ-DEFAULT-FLOAT-FORMAT*",
        "*READ-EVAL*",
        "*READ-SUPPRESS*",
        "*READTABLE*",
        "NCL-EXT::*PRINT-CIRCLE-NOT-SHARED*",
        "NCL-EXT::*PRINT-VECTOR-LENGTH*",
    ]);
    names.push("LET");
    let preinterned = names
        .into_iter()
        .take(preinterned_symbols)
        .map(|name| symbol(&mut ctx, &runtime, name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut roots = vec![Word::NIL; 8_192];
    roots[0] = input;
    roots[1..=preinterned.len()].copy_from_slice(&preinterned);

    ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        standard_io_reserve_heap_cells(
            ctx,
            &runtime,
            roots,
            1 + preinterned.len(),
            released_cells,
        )?;

        let input_args = [roots[0]];
        let args = ncl_object::BuiltinArgs::new(&input_args);
        let expand =
            callback_for("WITH-STANDARD-IO-SYNTAX").ok_or(ObjectError::UndefinedFunction)?;
        expand(ctx, &runtime, &args, &mut ncl_object::MultipleValues::new()).map(|_| ())
    })
}

#[test]
fn standard_io_syntax_propagates_heap_capacity_boundaries() {
    let cases = [
        ("first control symbol", 0, 0),
        ("KEYWORD::UPCASE symbol", 3, 7),
        ("SINGLE-FLOAT symbol", 13, 6),
        ("FIND-PACKAGE symbol", 14, 3),
        ("CL-USER string", 23, 3),
        ("CL-USER form list", 32, 3),
        ("COPY-PPRINT-DISPATCH symbol", 25, 30),
        ("pprint dispatch list", 68, 3),
        ("COPY-READTABLE symbol", 91, 3),
        ("readtable list", 96, 3),
        ("QUOTE symbol", 100, 3),
        ("float format list", 116, 3),
        ("binding symbol", 117, 30),
        ("aggregate bindings list", 136, 31),
        ("final binding form list", 145, 29),
        ("LET symbol", 163, 30),
        ("outer expansion list", 168, 31),
    ];

    for (case, released_cells, preinterned_symbols) in cases {
        let error = standard_io_expansion_with_released_cells(released_cells, preinterned_symbols)
            .expect_err("the selected allocation must report heap exhaustion");
        assert!(
            matches!(error, ObjectError::Storage(_)),
            "{case}: expected a storage condition, got {error:?}"
        );
        assert_eq!(
            format!("{error:?}"),
            "Storage(CapacityExceeded)",
            "{case}: reserve={released_cells}, preinterned_symbols={preinterned_symbols}"
        );
    }

    standard_io_expansion_with_released_cells(512, 31)
        .expect("512 one-word cells must complete the fully pre-interned expansion");
}
