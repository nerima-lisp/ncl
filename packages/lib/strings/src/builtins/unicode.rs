fn unicode_string_chars(ctx: &ThreadContext, value: Word) -> Result<Vec<char>, ObjectError> {
    (0..string_length(ctx, value)?)
        .map(|index| string_ref(ctx, value, index))
        .collect()
}

fn make_text(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    chars: &[char],
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let result = scope.make_string(runtime, chars)?;
    Ok(scope.get(result).as_word())
}

fn make_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let locals = values
        .iter()
        .copied()
        .map(ncl_object::Local::from_word)
        .collect::<Vec<_>>();
    let handles = scope.root_many(&locals);
    let result = scope.make_simple_vector(runtime, &handles)?;
    Ok(scope.get(result).as_word())
}

#[derive(Clone, Copy)]
enum Normalization {
    Canonical,
    Compatibility,
}

#[derive(Clone, Copy)]
enum Composition {
    Compose,
    Decompose,
}

impl Normalization {
    const fn accepts_compatibility(self, entry: bool) -> bool {
        matches!(self, Self::Compatibility) || !entry
    }
}

fn decompose_char(value: char, normalization: Normalization, output: &mut Vec<char>) {
    let codepoint = value as u32;
    if (0xAC00..0xD7A4).contains(&codepoint) {
        let index = codepoint - 0xAC00;
        output.push(char::from_u32(0x1100 + index / 588).unwrap_or(value));
        output.push(char::from_u32(0x1161 + (index % 588) / 28).unwrap_or(value));
        if !index.is_multiple_of(28) {
            output.push(char::from_u32(0x11A7 + index % 28).unwrap_or(value));
        }
        return;
    }
    if let Some((_, is_compatibility, values)) = unicode_data::DECOMPOSITIONS
        .iter()
        .flat_map(|entries| entries.iter())
        .find(|entry| entry.0 == codepoint)
        && normalization.accepts_compatibility(*is_compatibility)
    {
        for &part in *values {
            if let Some(part) = char::from_u32(part) {
                decompose_char(part, normalization, output);
            }
        }
        return;
    }
    output.push(value);
}

fn combining_class(value: char) -> u8 {
    unicode_data::COMBINING_CLASSES
        .iter()
        .flat_map(|entries| entries.iter())
        .find(|entry| entry.0 == value as u32)
        .map_or(0, |entry| entry.1)
}

fn normalize(
    chars: &[char],
    normalization: Normalization,
    composition: Composition,
) -> Vec<char> {
    let mut decomposed = Vec::new();
    for &value in chars {
        decompose_char(value, normalization, &mut decomposed);
    }
    let mut reordered = Vec::with_capacity(decomposed.len());
    for value in decomposed {
        let class = combining_class(value);
        if class == 0 {
            reordered.push(value);
        } else {
            let mut position = reordered.len();
            while position > 0 && combining_class(reordered[position - 1]) > class {
                position -= 1;
            }
            reordered.insert(position, value);
        }
    }
    if matches!(composition, Composition::Decompose) {
        return reordered;
    }
    let mut result = Vec::with_capacity(reordered.len());
    for value in reordered {
        if let Some(&starter) = result.last()
            && combining_class(value) != 0
            && let Some((_, _, composed)) = unicode_data::COMPOSITIONS
                .iter()
                .flat_map(|entries| entries.iter())
                .find(|entry| entry.0 == starter as u32 && entry.1 == value as u32)
        {
            result.pop();
            result.push(char::from_u32(*composed).unwrap_or(value));
            continue;
        }
        result.push(value);
    }
    result
}

fn transform_string<F>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    map: F,
) -> Result<Word, ObjectError>
where
    F: Fn(&[char]) -> Vec<char>,
{
    make_text(
        ctx,
        runtime,
        &map(&unicode_string_chars(ctx, args.required(0)?)?),
    )
}

fn normalize_nfc_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        normalize(
            s,
            Normalization::Canonical,
            Composition::Compose,
        )
    })
}
fn normalize_nfd_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        normalize(
            s,
            Normalization::Canonical,
            Composition::Decompose,
        )
    })
}
fn normalize_nfkc_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        normalize(
            s,
            Normalization::Compatibility,
            Composition::Compose,
        )
    })
}
fn normalize_nfkd_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        normalize(
            s,
            Normalization::Compatibility,
            Composition::Decompose,
        )
    })
}
fn full_upcase_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        s.iter().flat_map(|c| c.to_uppercase()).collect()
    })
}
fn full_downcase_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        s.iter().flat_map(|c| c.to_lowercase()).collect()
    })
}
fn full_titlecase_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    transform_string(ctx, runtime, args, |s| {
        let mut start = true;
        s.iter()
            .flat_map(|c| {
                let out: Vec<char> = if start {
                    c.to_uppercase().collect()
                } else {
                    c.to_lowercase().collect()
                };
                start = c.is_whitespace() || c.is_ascii_punctuation();
                out
            })
            .collect()
    })
}

fn string_to_utf8_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let bytes = unicode_string_chars(ctx, args.required(0)?)?
        .iter()
        .collect::<String>()
        .into_bytes();
    let values = bytes
        .into_iter()
        .map(|byte| Word::fixnum(i64::from(byte)))
        .collect::<Vec<_>>();
    make_vector(ctx, runtime, &values)
}
fn utf8_to_string_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let vector = args.required(0)?;
    let mut bytes = Vec::with_capacity(simple_vector_length(ctx, vector)?);
    for index in 0..simple_vector_length(ctx, vector)? {
        bytes.push(
            u8::try_from(
                simple_vector_ref(ctx, vector, index)?
                    .as_fixnum()
                    .ok_or(ObjectError::TypeError)?,
            )
            .map_err(|_| ObjectError::TypeError)?,
        );
    }
    let text = String::from_utf8(bytes).map_err(|_| ObjectError::TypeError)?;
    let chars: Vec<_> = text.chars().collect();
    make_text(ctx, runtime, &chars)
}

fn grapheme_boundaries_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let chars = unicode_string_chars(ctx, args.required(0)?)?;
    let mut boundaries = vec![Word::fixnum(0)];
    let mut regional_count = 0usize;
    for (index, &current) in chars.iter().enumerate() {
        let previous = index.checked_sub(1).and_then(|i| chars.get(i)).copied();
        let extend = general_category(current as u32)
            .is_some_and(|category| matches!(category, "Mn" | "Mc" | "Me"))
            || current == '\u{200D}'
            || (0xFE00..=0xFE0F).contains(&(current as u32));
        let regional = (0x1F1E6..=0x1F1FF).contains(&(current as u32));
        if previous
            .is_some_and(|p| {
                !extend && p != '\u{200D}' && (!regional || regional_count.is_multiple_of(2))
            })
        {
            boundaries.push(Word::fixnum(
                i64::try_from(index).map_err(|_| ObjectError::Layout)?,
            ));
        }
        regional_count = if regional { regional_count + 1 } else { 0 };
    }
    boundaries.push(Word::fixnum(
        i64::try_from(chars.len()).map_err(|_| ObjectError::Layout)?,
    ));
    make_vector(ctx, runtime, &boundaries)
}
