use ncl_object::{ObjectError, ObjectRef, ThreadContext, Word, classify_object};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SelectionOptions {
    pub start: usize,
    pub end: Option<usize>,
    pub from_end: bool,
    pub count: Option<usize>,
    pub key: Option<Word>,
    pub test: Option<Word>,
    pub test_not: Option<Word>,
}

pub fn parse_options(
    ctx: &ThreadContext,
    args: &[Word],
) -> Result<(Vec<Word>, SelectionOptions), ObjectError> {
    parse_options_with_minimum(ctx, args, 2)
}

pub fn parse_options_single(
    ctx: &ThreadContext,
    args: &[Word],
) -> Result<(Vec<Word>, SelectionOptions), ObjectError> {
    parse_options_with_minimum(ctx, args, 1)
}

fn parse_options_with_minimum(
    ctx: &ThreadContext,
    args: &[Word],
    minimum: usize,
) -> Result<(Vec<Word>, SelectionOptions), ObjectError> {
    let mut positional = Vec::new();
    let mut options = SelectionOptions::default();
    let mut index = 0;
    while index < args.len() {
        let argument = args.get(index).ok_or(ObjectError::Layout)?;
        let name = if let ObjectRef::Symbol(symbol) = classify_object(ctx, *argument) {
            let name = ncl_object::symbol_name(ctx, symbol)?;
            (0..ncl_object::string_length(ctx, name)?)
                .map(|i| ncl_object::string_ref(ctx, name, i))
                .collect::<Result<String, _>>()?
        } else {
            String::new()
        };
        let keyword = name.to_ascii_uppercase();
        if matches!(
            keyword.as_str(),
            "START" | "END" | "FROM-END" | "COUNT" | "KEY" | "TEST" | "TEST-NOT"
        ) {
            let value = *args.get(index + 1).ok_or(ObjectError::TypeError)?;
            if keyword == "START" {
                options.start = usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                    .map_err(|_| ObjectError::TypeError)?;
            } else if keyword == "END" {
                options.end = if value == Word::NIL {
                    None
                } else {
                    Some(
                        usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                            .map_err(|_| ObjectError::TypeError)?,
                    )
                };
            } else if keyword == "FROM-END" {
                options.from_end = value != Word::NIL;
            } else if keyword == "COUNT" {
                options.count = if value == Word::NIL {
                    None
                } else {
                    let count = value.as_fixnum().ok_or(ObjectError::TypeError)?;
                    Some(usize::try_from(count.max(0)).map_err(|_| ObjectError::TypeError)?)
                };
            } else if keyword == "KEY" {
                options.key = Some(value);
            } else if keyword == "TEST" {
                options.test = Some(value);
            } else if keyword == "TEST-NOT" {
                options.test_not = Some(value);
            }
            index += 2;
        } else {
            positional.push(*argument);
            index += 1;
        }
    }
    if positional.len() < minimum || options.test.is_some() && options.test_not.is_some() {
        return Err(ObjectError::TypeError);
    }
    Ok((positional, options))
}
