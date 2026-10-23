use ncl_object::{
    BuiltinArgs, FindStatus, MultipleValues, ObjectError, Package, Runtime, ThreadContext, Word,
};

use super::{package_arg, string_designator, with_rooted_words};

pub(super) fn status_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    status: FindStatus,
) -> Result<Word, ObjectError> {
    let keyword = Package::from_word(
        runtime
            .find_package(ctx, "KEYWORD")
            .ok_or(ObjectError::Layout)?,
    );
    let name = match status {
        FindStatus::Internal => "INTERNAL",
        FindStatus::External => "EXTERNAL",
        FindStatus::Inherited => "INHERITED",
    };
    keyword.intern(ctx, runtime, name).map(|(symbol, _)| symbol)
}

pub(super) fn intern(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = string_designator(ctx, args.required(0)?)?;
    let package = package_arg(ctx, runtime, args, 1)?;
    let name_word = name.as_word();
    let was_accessible = package.find_symbol(ctx, name_word)?.is_some();
    let keyword_package = runtime
        .find_package(ctx, "KEYWORD")
        .ok_or(ObjectError::Layout)?;
    let is_keyword_package = package.as_word() == keyword_package;
    let length = ncl_object::string_length(ctx, name_word)?;
    let name = (0..length)
        .map(|index| ncl_object::string_ref(ctx, name_word, index))
        .collect::<Result<String, _>>()?;
    let (symbol, status) = package.intern(ctx, runtime, &name)?;
    let mut symbol = symbol;
    let status = with_rooted_words(ctx, std::slice::from_mut(&mut symbol), |ctx, words| {
        if !was_accessible && is_keyword_package {
            if !package.export(ctx, runtime, name_word)? {
                return Err(ObjectError::Layout);
            }
            ncl_object::set_symbol_value(ctx, words[0], words[0])?; // check-added-lines: allow(index) from_mut above roots one symbol.
            ncl_object::set_symbol_constant(ctx, words[0], true)?;
            Ok(Word::NIL)
        } else if was_accessible {
            status_word(ctx, runtime, status)
        } else {
            Ok(Word::NIL)
        }
    })?;
    values.set(&[symbol, status]);
    Ok(symbol)
}
