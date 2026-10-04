use super::operations::{parse_namestring_builtin, parse_namestring_value};
use super::wildcard::translate_wildcards;
use super::{
    BuiltinArgs, MultipleValues, ObjectError, Package, Runtime, SLOTS, ThreadContext, Word, car,
    cdr, component_string, make_cons, make_pathname, make_string, namestring_value,
    pathname_designator, set_symbol_value, structure_ref, symbol_value, text,
};
use ncl_object::SetfExpansion;

pub fn logical_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    parse_namestring_builtin(ctx, runtime, args, values)
}

fn logical_translations_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    Ok(Package::from_word(package)
        .intern(ctx, runtime, "*LOGICAL-PATHNAME-TRANSLATIONS*")?
        .0)
}

fn logical_host(ctx: &ThreadContext, pathname: Word) -> Result<String, ObjectError> {
    component_string(ctx, structure_ref(ctx, pathname, 0)?)
}

pub fn logical_pathname_translations_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let host = logical_host(ctx, pathname)?;
    let symbol = logical_translations_symbol(ctx, runtime)?;
    let mut table = symbol_value(ctx, symbol)?;
    while table != Word::NIL {
        let entry = car(ctx, table)?;
        if text(ctx, car(ctx, entry)?)?.eq_ignore_ascii_case(&host) {
            if let Some(value) = args.get(1) {
                let pair = make_cons(ctx, runtime, car(ctx, entry)?, value)?;
                let rest = cdr(ctx, table)?;
                let updated = make_cons(ctx, runtime, pair, rest)?;
                set_symbol_value(ctx, symbol, updated)?;
            }
            return cdr(ctx, entry);
        }
        table = cdr(ctx, table)?;
    }
    if let Some(value) = args.get(1) {
        let host_word = make_string(ctx, runtime, &host.chars().collect::<Vec<_>>())?;
        let entry = make_cons(ctx, runtime, host_word, value)?;
        let table = symbol_value(ctx, symbol)?;
        let updated = make_cons(ctx, runtime, entry, table)?;
        set_symbol_value(ctx, symbol, updated)?;
        return Ok(value);
    }
    Ok(Word::NIL)
}

fn call_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let operator = Package::from_word(package).intern(ctx, runtime, name)?.0;
    ncl_object::with_roots(ctx, args, |ctx, roots| {
        let mut result = Word::NIL;
        for value in roots.iter().rev() {
            result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
                make_cons(ctx, runtime, **value, *result)
            })?;
        }
        ncl_object::with_root(ctx, &mut result, |ctx, result| {
            make_cons(ctx, runtime, operator, *result)
        })
    })
}

pub fn logical_pathname_translations_place(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<SetfExpansion, ObjectError> {
    let argument = *args.first().ok_or(ObjectError::TypeError)?;
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let package = runtime.ensure_package(ctx, "NCL")?;
    let temporary = Package::from_word(package).gensym(ctx, runtime)?;
    let store = Package::from_word(package).gensym(ctx, runtime)?;
    let access_form = call_form(ctx, runtime, "LOGICAL-PATHNAME-TRANSLATIONS", &[temporary])?;
    let store_form = call_form(
        ctx,
        runtime,
        "LOGICAL-PATHNAME-TRANSLATIONS",
        &[temporary, store],
    )?;
    Ok(SetfExpansion {
        temporary_variables: vec![temporary],
        value_forms: vec![argument],
        store_variables: vec![store],
        store_form,
        access_form,
    })
}

pub fn load_logical_pathname_translations_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let _ = pathname_designator(ctx, runtime, args.required(0)?)?;
    logical_pathname_translations_builtin(ctx, runtime, args, &mut MultipleValues::new())
}

pub fn translate_logical_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = logical_pathname_builtin(ctx, runtime, args, values)?;
    let host = logical_host(ctx, pathname)?;
    let symbol = logical_translations_symbol(ctx, runtime)?;
    let mut table = symbol_value(ctx, symbol)?;
    while table != Word::NIL {
        let entry = car(ctx, table)?;
        if text(ctx, car(ctx, entry)?)?.eq_ignore_ascii_case(&host) {
            let rules = cdr(ctx, entry)?;
            if rules != Word::NIL {
                let rule = car(ctx, rules)?;
                let source = pathname_designator(ctx, runtime, car(ctx, rule)?)?;
                let target = pathname_designator(ctx, runtime, car(ctx, cdr(ctx, rule)?)?)?;
                let source_name = namestring_value(ctx, source)?;
                let target_name = namestring_value(ctx, target)?;
                let value = namestring_value(ctx, pathname)?;
                let Some(translated) = translate_wildcards(&source_name, &target_name, &value)
                else {
                    table = cdr(ctx, table)?;
                    continue;
                };
                let string = make_string(ctx, runtime, &translated.chars().collect::<Vec<_>>())?;
                return parse_namestring_value(ctx, runtime, string);
            }
        }
        table = cdr(ctx, table)?;
    }
    Ok(pathname)
}

pub fn compile_file_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let type_ = component_string(ctx, structure_ref(ctx, pathname, 4)?)?;
    if type_.eq_ignore_ascii_case("fasl") {
        return Ok(pathname);
    }
    let type_word = make_string(ctx, runtime, &"fasl".chars().collect::<Vec<_>>())?;
    let mut slots = [Word::NIL; SLOTS];
    for (index, slot) in slots.iter_mut().enumerate() {
        *slot = structure_ref(ctx, pathname, index)?;
    }
    slots[4] = type_word; // check-added-lines: allow(index) fixed pathname slot layout
    make_pathname(ctx, runtime, &slots)
}
