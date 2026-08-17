use ncl_syntax::{Form, FormKind, Span};

fn atom(value: &str) -> Form {
    Form::atom(value, Span::new(0, value.len()))
}

#[test]
fn spans_report_lengths_and_empty_ranges() {
    let cases = [
        (Span::new(2, 7), 5, false),
        (Span::new(2, 2), 0, true),
        (Span::new(7, 2), 0, true),
    ];

    for (span, expected_len, expected_empty) in cases {
        assert_eq!(span.len(), expected_len);
        assert_eq!(span.is_empty(), expected_empty);
    }
}

#[test]
fn form_constructors_render_every_form_kind() {
    let cases = vec![
        ("atom", atom("ATOM"), "ATOM"),
        (
            "string",
            Form::new(
                FormKind::String("line\n\"quoted\"".to_owned()),
                Span::new(0, 1),
            ),
            "\"line\\n\\\"quoted\\\"\"",
        ),
        (
            "character",
            Form::new(FormKind::Character('A'), Span::new(0, 1)),
            r"#\A",
        ),
        ("empty list", Form::list(Vec::new(), Span::new(0, 2)), "()"),
        (
            "list",
            Form::list(vec![atom("A"), atom("B")], Span::new(0, 5)),
            "(A B)",
        ),
        (
            "empty dotted list",
            Form::dotted_list(Vec::new(), atom("TAIL"), Span::new(0, 7)),
            "(. TAIL)",
        ),
        (
            "dotted list",
            Form::dotted_list(vec![atom("A")], atom("TAIL"), Span::new(0, 9)),
            "(A . TAIL)",
        ),
        (
            "empty vector",
            Form::new(FormKind::Vector(Vec::new()), Span::new(0, 2)),
            "#()",
        ),
        (
            "vector",
            Form::new(
                FormKind::Vector(vec![atom("A"), atom("B")]),
                Span::new(0, 6),
            ),
            "#(A B)",
        ),
    ];

    for (name, form, expected) in cases {
        assert_eq!(form.to_string(), expected, "form display: {name}");
    }
}
