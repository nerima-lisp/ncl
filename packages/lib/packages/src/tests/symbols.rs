#![allow(clippy::unwrap_used)]

use super::super::*;
use ncl_object::{
    make_string, make_symbol, symbol_name, symbol_value, FunctionObject, ObjectError,
};

fn read_string(ctx: &ThreadContext, word: Word) -> String {
    let length = ncl_object::string_length(ctx, word).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, word, index).unwrap())
        .collect()
}

#[test]
fn symbol_cells_and_plist_round_trip() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    register(&runtime).unwrap();
    let symbol = Package::from_word(runtime.ensure_package(&mut ctx, "N25-SYMBOL").unwrap())
        .intern(&mut ctx, &runtime, "X")
        .unwrap()
        .0;
    let set = FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "SET").unwrap())
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, set, &[symbol, Word::fixnum(7)]),
        Ok(Word::fixnum(7))
    );
    assert_eq!(symbol_value(&ctx, symbol), Ok(Word::fixnum(7)));
    let get = FunctionObject::try_from(runtime.function(&mut ctx, "COMMON-LISP", "GET").unwrap())
        .unwrap();
    let key = make_symbol(&mut ctx, &runtime, Word::NIL).unwrap();
    let value_pair = ncl_object::make_cons(&mut ctx, &runtime, Word::TRUE, Word::NIL).unwrap();
    let plist = ncl_object::make_cons(&mut ctx, &runtime, key, value_pair).unwrap();
    ctx.write_object_slot(symbol, ncl_object::symbol_offset::PLIST, plist)
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, get, &[symbol, key]),
        Ok(Word::TRUE)
    );
}

#[test]
fn make_symbol_builtin_creates_uninterned_symbol_and_checks_designator() -> Result<(), ObjectError>
{
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;
    let function = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "MAKE-SYMBOL")
            .ok_or(ObjectError::Layout)?,
    )?;
    let name = make_string(&mut ctx, &runtime, &['N', '2', '5'])?;
    let symbol = runtime.call_builtin(&mut ctx, function, &[name])?;
    let symbol_name = symbol_name(&ctx, symbol)?;
    assert_eq!(ncl_object::string_length(&ctx, symbol_name)?, 3);
    assert_eq!(ncl_object::string_ref(&ctx, symbol_name, 0)?, 'N');
    assert_eq!(ncl_object::string_ref(&ctx, symbol_name, 1)?, '2');
    assert_eq!(ncl_object::string_ref(&ctx, symbol_name, 2)?, '5');
    assert_eq!(ncl_object::symbol_package(&ctx, symbol)?, Word::NIL);
    assert_eq!(
        runtime.call_builtin(&mut ctx, function, &[Word::fixnum(25)]),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn symbol_predicates_and_accessors_cover_unbound_and_uninterned_paths() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;

    let package_word = runtime.ensure_package(&mut ctx, "N25-SYMBOL-PREDICATES")?;
    let package = Package::from_word(package_word);
    let (symbol, _) = package.intern(&mut ctx, &runtime, "ACCESS")?;
    let (builtin_symbol, _) = Package::from_word(
        runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?,
    )
    .intern(&mut ctx, &runtime, "SET")?;

    let symbolp = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "SYMBOLP")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, symbolp, &[symbol]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, symbolp, &[Word::fixnum(0)]),
        Ok(Word::NIL)
    );

    let fboundp = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "FBOUNDP")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, fboundp, &[builtin_symbol]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fboundp, &[symbol]),
        Ok(Word::NIL)
    );

    ctx.write_object_slot(symbol, ncl_object::symbol_offset::FUNCTION, Word::TRUE)?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, fboundp, &[symbol]),
        Ok(Word::TRUE)
    );
    let fmakunbound = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "FMAKUNBOUND")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, fmakunbound, &[symbol]),
        Ok(symbol)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fboundp, &[symbol]),
        Ok(Word::NIL)
    );

    let symbol_name_builtin = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "SYMBOL-NAME")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, symbol_name_builtin, &[symbol]),
        Ok(symbol_name(&ctx, symbol)?)
    );
    let symbol_package = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "SYMBOL-PACKAGE")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, symbol_package, &[symbol]),
        Ok(package_word)
    );
    let symbol_plist = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "SYMBOL-PLIST")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, symbol_plist, &[symbol]),
        Ok(Word::NIL)
    );
    let symbol_function = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "SYMBOL-FUNCTION")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, symbol_function, &[symbol]),
        Ok(Word::UNBOUND)
    );

    Ok(())
}

#[test]
fn copy_symbol_preserves_name_and_clears_symbol_state() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;

    let package_word = runtime.ensure_package(&mut ctx, "N25-COPY-SYMBOL")?;
    let symbol = Package::from_word(package_word)
        .intern(&mut ctx, &runtime, "SOURCE")?
        .0;
    let copy_symbol = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "COPY-SYMBOL")
            .ok_or(ObjectError::Layout)?,
    )?;
    let copy = runtime.call_builtin(&mut ctx, copy_symbol, &[symbol])?;

    assert_ne!(copy, symbol);
    assert_eq!(symbol_name(&ctx, copy)?, symbol_name(&ctx, symbol)?);
    assert_eq!(ncl_object::symbol_package(&ctx, copy)?, Word::NIL);
    assert_eq!(symbol_value(&ctx, copy)?, Word::UNBOUND);
    assert_eq!(ncl_object::symbol_plist(&ctx, copy)?, Word::NIL);
    Ok(())
}

#[test]
fn symbol_property_walk_and_name_designators_cover_remaining_paths() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;

    let package_word = runtime.ensure_package(&mut ctx, "N25-SYMBOL-WALK")?;
    let symbol = Package::from_word(package_word)
        .intern(&mut ctx, &runtime, "PROPERTIES")?
        .0;
    let first_indicator = make_symbol(&mut ctx, &runtime, Word::NIL)?;
    let second_indicator = make_symbol(&mut ctx, &runtime, Word::NIL)?;
    let first_value = Word::fixnum(11);
    let second_value = Word::fixnum(22);
    let second_pair = ncl_object::make_cons(&mut ctx, &runtime, second_value, Word::NIL)?;
    let second_property = ncl_object::make_cons(&mut ctx, &runtime, second_indicator, second_pair)?;
    let first_pair = ncl_object::make_cons(&mut ctx, &runtime, first_value, second_property)?;
    let plist = ncl_object::make_cons(&mut ctx, &runtime, first_indicator, first_pair)?;
    ctx.write_object_slot(symbol, ncl_object::symbol_offset::PLIST, plist)?;

    let get = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GET")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, get, &[symbol, second_indicator]),
        Ok(second_value)
    );
    let remprop = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "REMPROP")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, remprop, &[symbol, second_indicator]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, get, &[symbol, first_indicator]),
        Ok(first_value)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, get, &[symbol, second_indicator]),
        Ok(Word::NIL)
    );

    let gensym = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GENSYM")
            .ok_or(ObjectError::Layout)?,
    )?;
    let default_gensym = runtime.call_builtin(&mut ctx, gensym, &[])?;
    let default_name = symbol_name(&ctx, default_gensym)?;
    assert_eq!(read_string(&ctx, default_name), "G0");
    let character_gensym =
        runtime.call_builtin(&mut ctx, gensym, &[Word::character(u32::from('C'))])?;
    let character_name = symbol_name(&ctx, character_gensym)?;
    assert_eq!(read_string(&ctx, character_name), "C1");
    let prefix_name = make_string(&mut ctx, &runtime, &['S'])?;
    let prefix_symbol = make_symbol(&mut ctx, &runtime, prefix_name)?;
    let symbol_gensym = runtime.call_builtin(&mut ctx, gensym, &[prefix_symbol])?;
    let symbol_name_result = symbol_name(&ctx, symbol_gensym)?;
    assert_eq!(read_string(&ctx, symbol_name_result), "S2");

    let gentemp = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GENTEMP")
            .ok_or(ObjectError::Layout)?,
    )?;
    let default_temp = runtime.call_builtin(&mut ctx, gentemp, &[])?;
    assert_eq!(read_string(&ctx, symbol_name(&ctx, default_temp)?), "T3");
    assert_eq!(
        ncl_object::symbol_package(&ctx, default_temp)?,
        runtime
            .find_package(&ctx, "COMMON-LISP-USER")
            .ok_or(ObjectError::Layout)?
    );
    let package_name = make_string(
        &mut ctx,
        &runtime,
        &"N25-SYMBOL-WALK".chars().collect::<Vec<_>>(),
    )?;
    let a_prefix = make_string(&mut ctx, &runtime, &['A', '-'])?;
    let string_temp = runtime.call_builtin(&mut ctx, gentemp, &[a_prefix, package_name])?;
    assert_eq!(read_string(&ctx, symbol_name(&ctx, string_temp)?), "A-4");
    let package_symbol = make_symbol(&mut ctx, &runtime, package_name)?;
    let b_prefix = make_string(&mut ctx, &runtime, &['B', '-'])?;
    let symbol_temp = runtime.call_builtin(&mut ctx, gentemp, &[b_prefix, package_symbol])?;
    assert_eq!(read_string(&ctx, symbol_name(&ctx, symbol_temp)?), "B-5");
    Ok(())
}

#[test]
fn symbol_text_and_generated_name_edges_are_value_based() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;
    register(&runtime)?;

    let text_string = make_string(&mut ctx, &runtime, &['T', 'E', 'X', 'T'])?;
    let text_symbol = make_symbol(&mut ctx, &runtime, text_string)?;
    let text_cases = [
        ("string", text_string, Ok(String::from("TEXT"))),
        ("symbol", text_symbol, Ok(String::from("TEXT"))),
        (
            "character",
            Word::character(u32::from('!')),
            Ok(String::from("!")),
        ),
        ("invalid", Word::fixnum(1), Err(ObjectError::TypeError)),
    ];
    for (label, value, expected) in text_cases {
        assert_eq!(super::text(&ctx, value), expected, "text {label}");
    }

    let invalid_symbol_argument_builtins = [
        "BOUNDP",
        "FBOUNDP",
        "SYMBOL-FUNCTION",
        "SYMBOL-NAME",
        "SYMBOL-PACKAGE",
        "SYMBOL-PLIST",
        "SYMBOL-VALUE",
    ];
    for name in invalid_symbol_argument_builtins {
        let builtin = FunctionObject::try_from(
            runtime
                .function(&mut ctx, "COMMON-LISP", name)
                .ok_or(ObjectError::Layout)?,
        )?;
        assert_eq!(
            runtime.call_builtin(&mut ctx, builtin, &[Word::fixnum(2)]),
            Err(ObjectError::TypeError),
            "{name}"
        );
    }

    let gensym = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GENSYM")
            .ok_or(ObjectError::Layout)?,
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, gensym, &[Word::fixnum(3)]),
        Err(ObjectError::TypeError)
    );

    let gentemp = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "GENTEMP")
            .ok_or(ObjectError::Layout)?,
    )?;
    let prefix = make_string(
        &mut ctx,
        &runtime,
        &['C', 'O', 'L', 'L', 'I', 'D', 'E', '-'],
    )?;
    let missing_package = make_string(
        &mut ctx,
        &runtime,
        &[
            'N', 'C', 'L', '-', 'N', 'O', '-', 'P', 'A', 'C', 'K', 'A', 'G', 'E',
        ],
    )?;
    assert_eq!(
        runtime.call_builtin(&mut ctx, gentemp, &[prefix, missing_package]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, gentemp, &[prefix, Word::fixnum(4)]),
        Err(ObjectError::TypeError)
    );

    let package = runtime.ensure_package(&mut ctx, "N25-GENTEMP-COLLISION")?;
    Package::from_word(package).intern(&mut ctx, &runtime, "COLLIDE-0")?;
    let collision_prefix = make_string(
        &mut ctx,
        &runtime,
        &['C', 'O', 'L', 'L', 'I', 'D', 'E', '-'],
    )?;
    let generated = runtime.call_builtin(&mut ctx, gentemp, &[collision_prefix, package])?;
    assert_eq!(
        read_string(&ctx, symbol_name(&ctx, generated)?),
        "COLLIDE-1"
    );
    Ok(())
}
