pub fn default_table(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    let mut entry = make_cons(ctx, runtime, Word::TRUE, Word::NIL)?;
    let token = push_root(ctx, &mut entry);
    let result = make_cons(ctx, runtime, entry, Word::NIL);
    let _ = pop_root(ctx, token);
    result
}

/// Return the function `table` associates with `object`, or `NIL`.
///
/// A `T` entry matches every object; any other specifier matches by identity.
/// Type-specifier matching needs `ncl-types`, which is not on `main` yet, so
/// non-`T` entries compare with `eq`.
///
/// # Errors
///
/// Returns an [`ObjectError`] when a list accessor fails.
#[must_use = "the dispatch function is the result"]
pub fn pprint_dispatch(
    ctx: &mut ThreadContext,
    object: Word,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut cursor = table;
    while cursor != Word::NIL {
        let entry = car(ctx, cursor)?;
        if entry.is_cons() {
            let specifier = car(ctx, entry)?;
            if matches_type_specifier(ctx, object, specifier)? {
                let payload = cdr(ctx, entry)?;
                if payload.is_cons() {
                    let candidate = car(ctx, payload)?;
                    let rest = cdr(ctx, payload)?;
                    if rest.as_fixnum().is_some() {
                        return Ok(candidate);
                    }
                }
                return Ok(payload);
            }
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(Word::NIL)
}

/// Return `table` with `(type_specifier . function)` prepended.
///
/// The new entry shadows any earlier entry with the same specifier, because
/// lookup returns the first match.
///
/// # Errors
///
/// Returns an [`ObjectError`] when an entry cannot be allocated.
#[must_use = "the updated table is the result"]
pub fn set_pprint_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    type_specifier: Word,
    function: Word,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut entry = make_cons(ctx, runtime, type_specifier, function)?;
    let token = push_root(ctx, &mut entry);
    let result = make_cons(ctx, runtime, entry, table);
    let _ = pop_root(ctx, token);
    result
}

/// Add a priority-aware dispatch entry. Higher priorities are consulted first.
///
/// # Errors
///
/// Returns an [`ObjectError`] if the table cannot be traversed or a new entry
/// cannot be allocated.
pub fn set_pprint_dispatch_with_priority(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    type_specifier: Word,
    function: Word,
    priority: i64,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut payload = make_cons(ctx, runtime, function, Word::fixnum(priority))?;
    let payload_token = push_root(ctx, &mut payload);
    let mut entry = make_cons(ctx, runtime, type_specifier, payload)?;
    let entry_token = push_root(ctx, &mut entry);
    let mut head = table;
    let head_token = push_root(ctx, &mut head);
    let mut cursor = table;
    while cursor != Word::NIL {
        let existing = car(ctx, cursor)?;
        let existing_payload = cdr(ctx, existing)?;
        let existing_priority = if existing_payload.is_cons() {
            let rest = cdr(ctx, existing_payload)?;
            rest.as_fixnum().unwrap_or(0)
        } else {
            0
        };
        if priority > existing_priority {
            break;
        }
        cursor = cdr(ctx, cursor)?;
    }
    if cursor == table {
        head = make_cons(ctx, runtime, entry, table)?;
    } else {
        let mut prefix = Word::NIL;
        let mut source = table;
        while source != cursor {
            let item = car(ctx, source)?;
            let token = push_root(ctx, &mut prefix);
            prefix = make_cons(ctx, runtime, item, prefix)?;
            let _ = pop_root(ctx, token);
            source = cdr(ctx, source)?;
        }
        let mut inserted = make_cons(ctx, runtime, entry, cursor)?;
        let token = push_root(ctx, &mut inserted);
        let mut reversed = prefix;
        while reversed != Word::NIL {
            let item = car(ctx, reversed)?;
            inserted = make_cons(ctx, runtime, item, inserted)?;
            reversed = cdr(ctx, reversed)?;
        }
        let _ = pop_root(ctx, token);
        head = inserted;
    }
    let _ = pop_root(ctx, head_token);
    let _ = pop_root(ctx, entry_token);
    let _ = pop_root(ctx, payload_token);
    Ok(head)
}

fn matches_type_specifier(
    ctx: &ThreadContext,
    object: Word,
    specifier: Word,
) -> Result<bool, ObjectError> {
    if specifier == Word::TRUE || specifier == object {
        return Ok(true);
    }
    let Ok(name) = symbol_name(ctx, specifier) else {
        return Ok(false);
    };
    let length = ncl_object::string_length(ctx, name)?;
    let mut text = String::with_capacity(length);
    for index in 0..length {
        text.push(ncl_object::string_ref(ctx, name, index)?);
    }
    let is_cons = object.is_cons();
    let kind = ncl_object::classify_object(ctx, object);
    let is_number = matches!(
        kind,
        ncl_object::ObjectRef::Fixnum(_)
            | ncl_object::ObjectRef::Bignum(_)
            | ncl_object::ObjectRef::Ratio(_)
            | ncl_object::ObjectRef::DoubleFloat(_)
            | ncl_object::ObjectRef::Complex(_)
    );
    let is_real = matches!(
        kind,
        ncl_object::ObjectRef::Fixnum(_)
            | ncl_object::ObjectRef::Bignum(_)
            | ncl_object::ObjectRef::Ratio(_)
            | ncl_object::ObjectRef::DoubleFloat(_)
    );
    let is_rational = matches!(
        kind,
        ncl_object::ObjectRef::Fixnum(_)
            | ncl_object::ObjectRef::Bignum(_)
            | ncl_object::ObjectRef::Ratio(_)
    );
    Ok(match text.to_ascii_uppercase().as_str() {
        "CONS" => is_cons,
        "LIST" => is_cons || object == Word::NIL,
        "SEQUENCE" => {
            is_cons
                || object == Word::NIL
                || matches!(
                    kind,
                    ncl_object::ObjectRef::SimpleVector(_)
                        | ncl_object::ObjectRef::SpecializedArray(_)
                        | ncl_object::ObjectRef::Array(_)
                )
        }
        "NULL" => object == Word::NIL,
        "ATOM" => !is_cons,
        "SYMBOL" => matches!(kind, ncl_object::ObjectRef::Symbol(_)),
        "INTEGER" => matches!(
            kind,
            ncl_object::ObjectRef::Fixnum(_) | ncl_object::ObjectRef::Bignum(_)
        ),
        "RATIONAL" => is_rational,
        "REAL" => is_real,
        "NUMBER" => is_number,
        "COMPLEX" => matches!(kind, ncl_object::ObjectRef::Complex(_)),
        "FLOAT" | "DOUBLE-FLOAT" => matches!(kind, ncl_object::ObjectRef::DoubleFloat(_)),
        "CHARACTER" => object.as_character().is_some(),
        "STRING" => matches!(kind, ncl_object::ObjectRef::String(_)),
        "VECTOR" => matches!(
            kind,
            ncl_object::ObjectRef::SimpleVector(_) | ncl_object::ObjectRef::SpecializedArray(_)
        ),
        "ARRAY" => matches!(
            kind,
            ncl_object::ObjectRef::SimpleVector(_)
                | ncl_object::ObjectRef::SpecializedArray(_)
                | ncl_object::ObjectRef::Array(_)
        ),
        "FUNCTION" => matches!(
            kind,
            ncl_object::ObjectRef::Function(_) | ncl_object::ObjectRef::Closure(_)
        ),
        _ => false, // check-added-lines: allow(wildcard) unhandled type specifier is not a dispatch match
    })
}

/// Return a shallow copy of a dispatch table.
///
/// # Errors
///
/// Returns an [`ObjectError`] when an entry cannot be allocated.
#[must_use = "the copied table is the result"]
pub fn copy_pprint_dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: Word,
) -> Result<Word, ObjectError> {
    let mut source = table;
    let mut copied = Word::NIL;
    let source_token = push_root(ctx, &mut source);
    let copied_token = push_root(ctx, &mut copied);
    let result = copy_entries(ctx, runtime, &mut source, &mut copied);
    let _ = pop_root(ctx, copied_token);
    let _ = pop_root(ctx, source_token);
    result?;
    Ok(copied)
}

fn copy_entries(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    source: &mut Word,
    copied: &mut Word,
) -> Result<(), ObjectError> {
    while *source != Word::NIL {
        let entry = car(ctx, *source)?;
        *copied = make_cons(ctx, runtime, entry, *copied)?;
        *source = cdr(ctx, *source)?;
    }
    Ok(())
}
