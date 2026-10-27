use std::collections::HashSet;

use ncl_object::{ObjectError, ObjectRef, Runtime, ThreadContext, Word, classify_object};

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SelectionOptionMode {
    Sequence,
    Tree,
    Pair,
}

pub fn parse_options(
    ctx: &ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<(Vec<Word>, SelectionOptions), ObjectError> {
    let (positional, options) =
        parse_options_with_minimum(ctx, runtime, args, 2, SelectionOptionMode::Sequence)?;
    Ok((positional, options.selection))
}

pub fn parse_options_single(
    ctx: &ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<(Vec<Word>, SelectionOptions), ObjectError> {
    let (positional, options) =
        parse_options_with_minimum(ctx, runtime, args, 1, SelectionOptionMode::Sequence)?;
    Ok((positional, options.selection))
}

pub fn parse_pair_options(
    ctx: &ThreadContext,
    runtime: &Runtime,
    args: &[Word],
) -> Result<(Vec<Word>, PairSelectionOptions), ObjectError> {
    parse_options_with_minimum(ctx, runtime, args, 2, SelectionOptionMode::Pair)
}

fn parse_index(value: Word) -> Result<usize, ObjectError> {
    usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)
}

fn is_selection_keyword(keyword: &str, mode: SelectionOptionMode) -> bool {
    match mode {
        SelectionOptionMode::Sequence => matches!(
            keyword,
            "START" | "END" | "FROM-END" | "COUNT" | "KEY" | "TEST" | "TEST-NOT"
        ),
        SelectionOptionMode::Tree => matches!(keyword, "KEY" | "TEST" | "TEST-NOT"),
        SelectionOptionMode::Pair => {
            matches!(keyword, "FROM-END" | "KEY" | "TEST" | "TEST-NOT")
        }
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

pub fn parse_options_with_minimum(
    ctx: &ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    minimum: usize,
    mode: SelectionOptionMode,
) -> Result<(Vec<Word>, PairSelectionOptions), ObjectError> {
    let positional = args.get(..minimum).ok_or(ObjectError::TypeError)?.to_vec();
    let keyword_package = runtime.find_package(ctx, "KEYWORD");
    let mut options = PairSelectionOptions::default();
    let mut allow_other_keys = None;
    let mut has_unrecognized_keyword = false;
    let mut seen_keywords = HashSet::new();
    let mut index = minimum;
    while index < args.len() {
        let argument = *args.get(index).ok_or(ObjectError::Layout)?;
        let value = *args.get(index + 1).ok_or(ObjectError::TypeError)?;
        let keyword = if let ObjectRef::Symbol(symbol) = classify_object(ctx, argument) {
            let package = ncl_object::symbol_package(ctx, symbol)?;
            if keyword_package == Some(package) {
                let name = ncl_object::symbol_name(ctx, symbol)?;
                (0..ncl_object::string_length(ctx, name)?)
                    .map(|i| ncl_object::string_ref(ctx, name, i))
                    .collect::<Result<String, _>>()?
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        let pair_bound = mode == SelectionOptionMode::Pair
            && matches!(keyword.as_str(), "START1" | "END1" | "START2" | "END2");
        let allow_other_keys_keyword = keyword == "ALLOW-OTHER-KEYS";
        if is_selection_keyword(&keyword, mode) || pair_bound || allow_other_keys_keyword {
            if mode != SelectionOptionMode::Sequence && !seen_keywords.insert(keyword.clone()) {
                index += 2;
                continue;
            }
            apply_option(&mut options, &keyword, value, &mut allow_other_keys)?;
        } else {
            if mode == SelectionOptionMode::Pair
                && !matches!(classify_object(ctx, argument), ObjectRef::Symbol(_))
            {
                return Err(ObjectError::TypeError);
            }
            has_unrecognized_keyword = true;
        }
        index += 2;
    }
    if options.selection.test.is_some() && options.selection.test_not.is_some()
        || has_unrecognized_keyword && allow_other_keys != Some(true)
    {
        return Err(ObjectError::TypeError);
    }
    Ok((positional, options))
}
