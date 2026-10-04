const TYPE_SPECIFIER_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("TYPE-SPECIFIER"),
    ty: ParameterType::Any,
};
const FUNCTION_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("FUNCTION"),
    ty: ParameterType::Any,
};
const PRIORITY_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("PRIORITY"),
    ty: ParameterType::Any,
};
const TABLE_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("TABLE"),
    ty: ParameterType::Any,
};
const PREFIX_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("PREFIX"),
    ty: ParameterType::Any,
};
const PER_LINE_PREFIX_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("PER-LINE-PREFIX"),
    ty: ParameterType::Any,
};
const SUFFIX_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("SUFFIX"),
    ty: ParameterType::Any,
};
const THUNK_PARAMETER: Parameter = Parameter {
    name: BuiltinName::new("THUNK"),
    ty: ParameterType::Any,
};

struct LogicalBlock {
    cursor: Word,
    count: usize,
    limit: Option<usize>,
    tag: Word,
    seen: HashSet<usize>,
}

static LOGICAL_BLOCKS: OnceLock<Mutex<HashMap<usize, Vec<LogicalBlock>>>> = OnceLock::new();

include!("pprint_logical_block.rs");

#[allow(clippy::too_many_lines, reason = "flat printer builtin dispatch table")]
fn register_pprint_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<(), ObjectError> {
    let (descriptor, function): (Builtin, ncl_object::RustBuiltin) = match name {
        "PPRINT-LOGICAL-BLOCK" => (
            Builtin {
                lambda_list: LambdaList::fixed(&[
                    STREAM_PARAMETER,
                    OBJECT_PARAMETER,
                    PREFIX_PARAMETER,
                    PER_LINE_PREFIX_PARAMETER,
                    SUFFIX_PARAMETER,
                    THUNK_PARAMETER,
                ]),
                convention: BuiltinConvention::Adapted,
            },
            pprint_logical_block,
        ),
        "PPRINT-POP" => (
            Builtin {
                lambda_list: LambdaList::fixed(&[]),
                convention: BuiltinConvention::Adapted,
            },
            pprint_pop,
        ),
        "PPRINT-EXIT-IF-LIST-EXHAUSTED" => (
            Builtin {
                lambda_list: LambdaList::fixed(&[]),
                convention: BuiltinConvention::Adapted,
            },
            pprint_exit_if_list_exhausted,
        ),
        "PPRINT" => (
            Builtin {
                lambda_list: LambdaList::with_optional(&[OBJECT_PARAMETER], &[STREAM_PARAMETER]),
                convention: BuiltinConvention::Adapted,
            },
            pprint,
        ),
        "PPRINT-DISPATCH" => (
            Builtin {
                lambda_list: LambdaList::with_optional(&[OBJECT_PARAMETER], &[TABLE_PARAMETER]),
                convention: BuiltinConvention::Adapted,
            },
            pprint_dispatch_builtin,
        ),
        "SET-PPRINT-DISPATCH" => (
            Builtin {
                lambda_list: LambdaList::with_optional(
                    &[TYPE_SPECIFIER_PARAMETER, FUNCTION_PARAMETER],
                    &[PRIORITY_PARAMETER, TABLE_PARAMETER],
                ),
                convention: BuiltinConvention::Adapted,
            },
            set_pprint_dispatch_builtin,
        ),
        "COPY-PPRINT-DISPATCH" => (
            Builtin {
                lambda_list: LambdaList::with_optional(&[], &[TABLE_PARAMETER]),
                convention: BuiltinConvention::Adapted,
            },
            copy_pprint_dispatch_builtin,
        ),
        "PPRINT-NEWLINE" => (
            Builtin {
                lambda_list: LambdaList::with_rest(&[], OBJECT_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            pprint_newline,
        ),
        "PPRINT-INDENT" => (
            Builtin {
                lambda_list: LambdaList::with_rest(&[], OBJECT_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            pprint_indent,
        ),
        "PPRINT-TAB" => (
            Builtin {
                lambda_list: LambdaList::with_rest(&[], OBJECT_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            pprint_tab,
        ),
        "PPRINT-FILL" | "PPRINT-LINEAR" | "PPRINT-TABULAR" => (
            Builtin {
                lambda_list: LambdaList::with_rest(&[], OBJECT_PARAMETER),
                convention: BuiltinConvention::Adapted,
            },
            pprint_object,
        ),
        _ => return Err(ObjectError::Layout), // check-added-lines: allow(wildcard) string dispatch rejects unknown names
    };
    let builtin_name = match name {
        "PPRINT-LOGICAL-BLOCK" => BuiltinName::new("PPRINT-LOGICAL-BLOCK"),
        "PPRINT-POP" => BuiltinName::new("PPRINT-POP"),
        "PPRINT-EXIT-IF-LIST-EXHAUSTED" => BuiltinName::new("PPRINT-EXIT-IF-LIST-EXHAUSTED"),
        "PPRINT" => BuiltinName::new("PPRINT"),
        "PPRINT-DISPATCH" => BuiltinName::new("PPRINT-DISPATCH"),
        "SET-PPRINT-DISPATCH" => BuiltinName::new("SET-PPRINT-DISPATCH"),
        "COPY-PPRINT-DISPATCH" => BuiltinName::new("COPY-PPRINT-DISPATCH"),
        "PPRINT-NEWLINE" => BuiltinName::new("PPRINT-NEWLINE"),
        "PPRINT-INDENT" => BuiltinName::new("PPRINT-INDENT"),
        "PPRINT-TAB" => BuiltinName::new("PPRINT-TAB"),
        "PPRINT-FILL" => BuiltinName::new("PPRINT-FILL"),
        "PPRINT-LINEAR" => BuiltinName::new("PPRINT-LINEAR"),
        "PPRINT-TABULAR" => BuiltinName::new("PPRINT-TABULAR"),
        _ => return Err(ObjectError::Layout), // check-added-lines: allow(wildcard) string dispatch rejects unknown names
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            if matches!(
                name,
                "PPRINT-LOGICAL-BLOCK" | "PPRINT-POP" | "PPRINT-EXIT-IF-LIST-EXHAUSTED"
            ) {
                BuiltinPackage::NclExt
            } else {
                BuiltinPackage::CommonLisp
            },
            builtin_name,
        ),
        BuiltinImplementation::adapted(descriptor, function, print_arguments),
    )?;
    Ok(())
}

fn logical_blocks() -> &'static Mutex<HashMap<usize, Vec<LogicalBlock>>> {
    LOGICAL_BLOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn logical_block_key(ctx: &mut ThreadContext) -> usize {
    std::ptr::from_mut(ctx).addr()
}

fn pprint(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let options = PrintOptions::from_specials(ctx, runtime).with_pretty(true);
    let _ = print_object(ctx, runtime, args, options, false)?;
    Ok(Word::NIL)
}

fn dispatch_table(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    supplied: Option<Word>,
) -> Result<Word, ObjectError> {
    supplied.filter(|table| *table != Word::NIL).map_or_else(
        || {
            let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
            with_root(ctx, &mut package.clone(), |ctx, package| {
                let (mut symbol, _) =
                    Package::from_word(*package).intern(ctx, runtime, "*PRINT-PPRINT-DISPATCH*")?;
                with_root(ctx, &mut symbol, |ctx, symbol| symbol_value(ctx, *symbol))
            })
        },
        Ok,
    )
}

fn pprint_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let table = dispatch_table(ctx, runtime, args.get(1))?;
    pprint_dispatch(ctx, object, table)
}

fn set_pprint_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let type_specifier = args.required(0)?;
    let function = args.required(1)?;
    let table = dispatch_table(ctx, runtime, args.get(3))?;
    let _priority = args.get(2);
    let priority = args.get(2).and_then(Word::as_fixnum).unwrap_or(0);
    let updated =
        set_pprint_dispatch_with_priority(ctx, runtime, type_specifier, function, priority, table)?;
    if args.get(3).is_none() {
        let package = runtime.ensure_package(ctx, "COMMON-LISP")?;
        with_root(ctx, &mut package.clone(), |ctx, package| {
            let (mut symbol, _) =
                Package::from_word(*package).intern(ctx, runtime, "*PRINT-PPRINT-DISPATCH*")?;
            with_root(ctx, &mut symbol, |ctx, symbol| {
                set_symbol_value(ctx, *symbol, updated)
            })
        })?;
    }
    Ok(Word::NIL)
}

fn copy_pprint_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let table = dispatch_table(ctx, runtime, args.get(0))?;
    copy_pprint_dispatch(ctx, runtime, table)
}

fn pprint_newline(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = output_stream(ctx, runtime, None)?;
    let kind = args.required(0)?;
    let kind = symbol_text(ctx, kind)?.to_ascii_uppercase();
    with_layout_state(stream, |state| {
        let newline = match kind.as_str() {
            "MANDATORY" => NewlineKind::Mandatory,
            "MISER" => NewlineKind::Miser,
            "FILL" => NewlineKind::Fill,
            "LINEAR" => NewlineKind::Linear,
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown newline kinds
        };
        state.pending = Some(newline);
        if newline == NewlineKind::Mandatory {
            flush_pending(ctx, runtime, stream, state, true)?;
        }
        Ok(())
    })?;
    Ok(Word::NIL)
}

fn pprint_indent(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = output_stream(ctx, runtime, None)?;
    let amount = args
        .required(1)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    with_layout_state(stream, |state| {
        state.indent = usize::try_from(amount).unwrap_or(0);
        Ok(())
    })?;
    Ok(Word::NIL)
}

fn pprint_tab(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = output_stream(ctx, runtime, None)?;
    let column = usize::try_from(
        args.required(1)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    let increment = usize::try_from(
        args.required(2)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    with_layout_state(stream, |state| {
        flush_pending(ctx, runtime, stream, state, false)?;
        let target = match symbol_text(ctx, args.required(0)?)?
            .to_ascii_uppercase()
            .as_str()
        {
            "ABSOLUTE" => {
                if state.column < column || increment == 0 {
                    column
                } else {
                    column + (state.column - column) / increment * increment + increment
                }
            }
            "RELATIVE" => state.column + column,
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown tab kinds
        };
        write_spaces(ctx, runtime, stream, target.saturating_sub(state.column))?;
        state.column = target;
        Ok(())
    })?;
    Ok(Word::NIL)
}

fn pprint_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let stream = output_stream(ctx, runtime, args.get(1))?;
    let options = PrintOptions::from_specials(ctx, runtime).with_pretty(true);
    let rendered =
        write_to_string(ctx, runtime, object, &options).map_err(|error| print_error(&error))?;
    with_root(ctx, &mut rendered.clone(), |ctx, rendered| {
        with_layout_state(stream, |state| {
            flush_pending(ctx, runtime, stream, state, false)?;
            call_builtin(ctx, runtime, "WRITE-STRING", &[*rendered, stream])?;
            state.column = rendered_text_column(ctx, *rendered)?;
            Ok(())
        })
    })?;
    Ok(Word::NIL)
}

fn with_layout_state<T>(
    stream: Word,
    operation: impl FnOnce(&mut LayoutState) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let states = LAYOUT_STATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut states = states.lock().map_err(|_| ObjectError::Layout)?;
    let mut state = states.remove(&stream.address()).unwrap_or_default();
    let result = operation(&mut state);
    states.insert(stream.address(), state);
    result
}

fn flush_pending(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    stream: Word,
    state: &mut LayoutState,
    mandatory: bool,
) -> Result<(), ObjectError> {
    let Some(kind) = state.pending.take() else {
        return Ok(());
    };
    if mandatory || matches!(kind, NewlineKind::Mandatory) {
        call_builtin(
            ctx,
            runtime,
            "WRITE-CHAR",
            &[Word::character(u32::from('\n')), stream],
        )?;
        state.column = 0;
        write_spaces(ctx, runtime, stream, state.indent)?;
        state.column = state.indent;
    } else {
        call_builtin(
            ctx,
            runtime,
            "WRITE-CHAR",
            &[Word::character(u32::from(' ')), stream],
        )?;
        state.column += 1;
    }
    Ok(())
}

fn write_spaces(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    stream: Word,
    count: usize,
) -> Result<(), ObjectError> {
    for _ in 0..count {
        call_builtin(
            ctx,
            runtime,
            "WRITE-CHAR",
            &[Word::character(u32::from(' ')), stream],
        )?;
    }
    Ok(())
}

fn symbol_text(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    let name = symbol_name(ctx, symbol)?;
    let length = ncl_object::string_length(ctx, name)?;
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect()
}

fn rendered_text_column(ctx: &ThreadContext, string: Word) -> Result<usize, ObjectError> {
    let length = ncl_object::string_length(ctx, string)?;
    let mut column = 0;
    for index in 0..length {
        if ncl_object::string_ref(ctx, string, index)? == '\n' {
            column = 0;
        } else {
            column += 1;
        }
    }
    Ok(column)
}
