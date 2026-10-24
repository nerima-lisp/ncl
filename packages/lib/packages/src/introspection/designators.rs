use ncl_object::{
    ObjectError, ObjectRef, Package, Runtime, ThreadContext, Word, classify_object, symbol_name,
};

pub fn string_designator_text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let object = classify_object(ctx, word);
    if let ObjectRef::String(_) = object {
        let length = ncl_object::string_length(ctx, word)?;
        (0..length)
            .map(|index| ncl_object::string_ref(ctx, word, index))
            .collect()
    } else if let ObjectRef::Symbol(symbol) = object {
        let name = symbol_name(ctx, symbol)?;
        let length = ncl_object::string_length(ctx, name)?;
        (0..length)
            .map(|index| ncl_object::string_ref(ctx, name, index))
            .collect()
    } else if let ObjectRef::Character(character) = object {
        char::from_u32(character)
            .map(|character| character.to_string())
            .ok_or(ObjectError::TypeError)
    } else {
        Err(ObjectError::TypeError)
    }
}

pub fn string_designator_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Word, ObjectError> {
    let object = classify_object(ctx, word);
    if let ObjectRef::String(_) = object {
        Ok(word)
    } else if let ObjectRef::Symbol(symbol) = object {
        symbol_name(ctx, symbol)
    } else if let ObjectRef::Character(character) = object {
        let character = char::from_u32(character).ok_or(ObjectError::TypeError)?;
        ncl_object::make_string(ctx, runtime, &[character])
    } else {
        Err(ObjectError::TypeError)
    }
}

pub fn package_designator(
    ctx: &ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Package, ObjectError> {
    if let ObjectRef::Package(package) = classify_object(ctx, word) {
        return Ok(Package::from_word(package));
    }
    let name = string_designator_text(ctx, word)?;
    runtime
        .find_package(ctx, &name)
        .map(Package::from_word)
        .ok_or(ObjectError::TypeError)
}
