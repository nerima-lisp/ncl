use crate::builtins::{
    arity, array_option_name, array_total_size_for, flatten_array_contents, index_argument,
    parse_array_dimensions, type_error,
};
use crate::{RuntimeError, SharedElements, Value};

#[derive(Clone, Default)]
pub(crate) struct ArrayOptions {
    pub(crate) initial_element: Option<Value>,
    pub(crate) initial_contents: Option<Value>,
    pub(crate) element_type: Option<Value>,
    pub(crate) adjustable: bool,
    pub(crate) adjustable_specified: bool,
    pub(crate) fill_pointer: Option<Value>,
    pub(crate) fill_pointer_specified: bool,
    pub(crate) displaced_to: Option<Value>,
    pub(crate) displaced_index_offset: usize,
}

fn parse_keyword_options(
    function: &str,
    arguments: &[Value],
) -> Result<ArrayOptions, RuntimeError> {
    if !arguments.len().is_multiple_of(2) {
        return Err(arity(function, "keyword/value pairs", arguments.len()));
    }
    let mut options = ArrayOptions::default();
    for pair in arguments.as_chunks::<2>().0 {
        let name = array_option_name(function, &pair[0])?;
        match name.as_str() {
            "INITIAL-ELEMENT" => {
                if options.initial_contents.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: format!(
                            "{function} cannot combine :initial-element and :initial-contents"
                        ),
                        span: None,
                    });
                }
                options.initial_element = Some(pair[1].clone());
            }
            "INITIAL-CONTENTS" => {
                if options.initial_element.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: format!(
                            "{function} cannot combine :initial-element and :initial-contents"
                        ),
                        span: None,
                    });
                }
                options.initial_contents = Some(pair[1].clone());
            }
            "ELEMENT-TYPE" => options.element_type = Some(pair[1].clone()),
            "ADJUSTABLE" => {
                options.adjustable = pair[1].is_truthy();
                options.adjustable_specified = true;
            }
            "FILL-POINTER" => {
                options.fill_pointer = Some(pair[1].clone());
                options.fill_pointer_specified = true;
            }
            "DISPLACED-TO" => options.displaced_to = Some(pair[1].clone()),
            "DISPLACED-INDEX-OFFSET" => {
                options.displaced_index_offset = index_argument(function, &pair[1])?;
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    Ok(options)
}

fn parse_array_options(function: &str, arguments: &[Value]) -> Result<ArrayOptions, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    parse_keyword_options(function, &arguments[1..])
}

fn fill_pointer_value(
    function: &str,
    value: Option<&Value>,
    total_size: usize,
) -> Result<Option<usize>, RuntimeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if matches!(value, Value::Nil | Value::Boolean(false)) {
        return Ok(None);
    }
    let pointer = if value.symbol_name() == Some("T") {
        total_size
    } else {
        index_argument(function, value)?
    };
    if pointer > total_size {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} fill pointer exceeds vector length"),
            span: None,
        });
    }
    Ok(Some(pointer))
}

fn make_array_value(
    function: &str,
    dimensions: Vec<usize>,
    options: ArrayOptions,
) -> Result<Value, RuntimeError> {
    let total_size = array_total_size_for(function, &dimensions)?;
    let element_type = options.element_type.unwrap_or_else(|| Value::symbol("T"));
    let fill_pointer = fill_pointer_value(function, options.fill_pointer.as_ref(), total_size)?;
    if fill_pointer.is_some() && dimensions.len() != 1 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} fill pointer requires a one-dimensional array"),
            span: None,
        });
    }
    if options.displaced_to.is_some()
        && (options.initial_element.is_some() || options.initial_contents.is_some())
    {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} cannot initialize a displaced array"),
            span: None,
        });
    }
    let elements = if let Some(source) = options.displaced_to {
        let source_value = source.clone();
        let source = source
            .array_storage()
            .ok_or_else(|| type_error(function, "an array for :displaced-to", &source_value))?;
        source
            .displaced_view(
                source_value,
                options.displaced_index_offset,
                total_size,
                element_type.clone(),
                fill_pointer,
                options.adjustable,
            )
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} displaced array exceeds its source"),
                span: None,
            })?
    } else {
        let values = if let Some(contents) = options.initial_contents {
            let mut values = Vec::with_capacity(total_size);
            flatten_array_contents(function, &contents, &dimensions, &mut values)?;
            values
        } else {
            vec![options.initial_element.unwrap_or(Value::Nil); total_size]
        };
        SharedElements::new_with_options(values, fill_pointer, options.adjustable, element_type)
    };
    if dimensions.len() == 1 {
        Ok(Value::vector_with_elements(elements))
    } else {
        Ok(Value::array_with_elements(dimensions, elements))
    }
}

#[expect(clippy::unnecessary_wraps)]
pub fn vector(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::vector(arguments.to_vec()))
}

pub fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let options = parse_array_options("make-array", arguments)?;
    make_array_value("make-array", dimensions, options)
}

pub(crate) fn make_array_from_dimensions(
    function: &str,
    dimensions: Vec<usize>,
    options: ArrayOptions,
) -> Result<Value, RuntimeError> {
    make_array_value(function, dimensions, options)
}

pub(crate) fn parse_options_for_adjust_array(
    arguments: &[Value],
) -> Result<ArrayOptions, RuntimeError> {
    parse_keyword_options("adjust-array", arguments)
}
