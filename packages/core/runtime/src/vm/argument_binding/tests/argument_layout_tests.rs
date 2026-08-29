use crate::Value;

use super::{argument_layout, function};

#[test]
fn argument_layout_handles_optional_keyword_and_rest_shapes() {
    let mut optional = function(vec![]);
    optional.parameters.push("required".to_string());
    optional.optional.push(ncl_compiler::OptionalParameter {
        name: "optional".to_string(),
        name_escaped: false,
        default_function: 0,
        supplied_p: None,
        supplied_p_escaped: None,
    });
    optional.has_keyword_section = true;

    let layouts = [
        (&optional, vec![Value::Integer(1)], (0, 1)),
        (
            &optional,
            vec![Value::Integer(1), Value::Keyword("key".to_string().into())],
            (0, 1),
        ),
        (
            &optional,
            vec![Value::Integer(1), Value::Integer(2)],
            (1, 2),
        ),
    ];

    for (function, arguments, expected) in layouts {
        assert!(matches!(
            argument_layout(function, &arguments),
            Ok(layout) if layout == expected
        ));
    }
}
