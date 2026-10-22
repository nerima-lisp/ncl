use std::collections::HashSet;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PairSelectionOptions {
    pub(crate) selection: SelectionOptions,
    pub(crate) start1: Option<usize>,
    pub(crate) end1: End1Bound,
    pub(crate) start2: Option<usize>,
    pub(crate) end2: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum End1Bound {
    #[default]
    Omitted,
    Unbounded,
    Index(usize),
}

pub fn parse_options(
    ctx: &ThreadContext,
    args: &[Word],
) -> Result<(Vec<Word>, SelectionOptions), ObjectError> {
    let (positional, options) = parse_options_with_minimum(ctx, args, 2, false)?;
    Ok((positional, options.selection))
}

pub fn parse_options_single(
    ctx: &ThreadContext,
    args: &[Word],
) -> Result<(Vec<Word>, SelectionOptions), ObjectError> {
    let (positional, options) = parse_options_with_minimum(ctx, args, 1, false)?;
    Ok((positional, options.selection))
}

pub fn parse_pair_options(
    ctx: &ThreadContext,
    args: &[Word],
) -> Result<(Vec<Word>, PairSelectionOptions), ObjectError> {
    parse_options_with_minimum(ctx, args, 2, true)
}

fn parse_index(value: Word) -> Result<usize, ObjectError> {
    usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)
}

fn is_selection_keyword(keyword: &str, pair_operation: bool) -> bool {
    if pair_operation {
        matches!(keyword, "FROM-END" | "KEY" | "TEST" | "TEST-NOT")
    } else {
        matches!(
            keyword,
            "START" | "END" | "FROM-END" | "COUNT" | "KEY" | "TEST" | "TEST-NOT"
        )
    }
}

fn apply_option(
    options: &mut PairSelectionOptions,
    keyword: &str,
    value: Word,
    allow_other_keys: &mut Option<bool>,
) -> Result<(), ObjectError> {
    if keyword == "START" {
        options.selection.start = parse_index(value)?;
    } else if keyword == "END" {
        options.selection.end = if value == Word::NIL {
            None
        } else {
            Some(parse_index(value)?)
        };
    } else if keyword == "START1" {
        options.start1 = Some(parse_index(value)?);
    } else if keyword == "END1" {
        options.end1 = if value == Word::NIL {
            End1Bound::Unbounded
        } else {
            End1Bound::Index(parse_index(value)?)
        };
    } else if keyword == "START2" {
        options.start2 = Some(parse_index(value)?);
    } else if keyword == "END2" {
        options.end2 = if value == Word::NIL {
            None
        } else {
            Some(parse_index(value)?)
        };
    } else if keyword == "FROM-END" {
        options.selection.from_end = value != Word::NIL;
    } else if keyword == "COUNT" {
        options.selection.count = if value == Word::NIL {
            None
        } else {
            let count = value.as_fixnum().ok_or(ObjectError::TypeError)?;
            Some(usize::try_from(count.max(0)).map_err(|_| ObjectError::TypeError)?)
        };
    } else if keyword == "KEY" {
        options.selection.key = Some(value);
    } else if keyword == "TEST" {
        options.selection.test = Some(value);
    } else if keyword == "TEST-NOT" {
        options.selection.test_not = Some(value);
    } else if keyword == "ALLOW-OTHER-KEYS" && allow_other_keys.is_none() {
        *allow_other_keys = Some(value != Word::NIL);
    }
    Ok(())
}

fn parse_options_with_minimum(
    ctx: &ThreadContext,
    args: &[Word],
    minimum: usize,
    pair_operation: bool,
) -> Result<(Vec<Word>, PairSelectionOptions), ObjectError> {
    let mut positional = Vec::new();
    let mut options = PairSelectionOptions::default();
    let mut allow_other_keys = None;
    let mut has_unrecognized_keyword = false;
    let mut seen_pair_keywords = HashSet::new();
    let mut index = 0;
    while index < args.len() {
        let argument = args.get(index).ok_or(ObjectError::Layout)?;
        if pair_operation && positional.len() < minimum {
            positional.push(*argument);
            index += 1;
            continue;
        }
        let name = if let ObjectRef::Symbol(symbol) = classify_object(ctx, *argument) {
            let name = ncl_object::symbol_name(ctx, symbol)?;
            (0..ncl_object::string_length(ctx, name)?)
                .map(|i| ncl_object::string_ref(ctx, name, i))
                .collect::<Result<String, _>>()?
        } else {
            String::new()
        };
        let keyword = name.to_ascii_uppercase();
        let pair_bound = matches!(keyword.as_str(), "START1" | "END1" | "START2" | "END2");
        let allow_other_keys_keyword = keyword == "ALLOW-OTHER-KEYS";
        if pair_bound && !pair_operation {
            return Err(ObjectError::TypeError);
        }
        if is_selection_keyword(&keyword, pair_operation)
            || pair_bound
            || pair_operation && allow_other_keys_keyword
        {
            if pair_operation && !seen_pair_keywords.insert(keyword.clone()) {
                index += 2;
                continue;
            }
            let value = *args.get(index + 1).ok_or(ObjectError::TypeError)?;
            apply_option(&mut options, &keyword, value, &mut allow_other_keys)?;
            index += 2;
        } else if pair_operation {
            if !matches!(classify_object(ctx, *argument), ObjectRef::Symbol(_)) {
                return Err(ObjectError::TypeError);
            }
            let _value = args.get(index + 1).ok_or(ObjectError::TypeError)?;
            has_unrecognized_keyword = true;
            index += 2;
        } else {
            positional.push(*argument);
            index += 1;
        }
    }
    if positional.len() < minimum
        || options.selection.test.is_some() && options.selection.test_not.is_some()
        || has_unrecognized_keyword && allow_other_keys != Some(true)
    {
        return Err(ObjectError::TypeError);
    }
    Ok((positional, options))
}
