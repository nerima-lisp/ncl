use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    pub kind: FormKind,
    pub span: Span,
}

impl Form {
    #[must_use]
    pub const fn new(kind: FormKind, span: Span) -> Self {
        Self { kind, span }
    }

    #[must_use]
    pub fn atom(value: impl Into<String>, span: Span) -> Self {
        Self::new(FormKind::Atom(value.into()), span)
    }

    #[must_use]
    pub fn list(items: Vec<Self>, span: Span) -> Self {
        Self::new(FormKind::List(items), span)
    }

    #[must_use]
    pub fn dotted_list(items: Vec<Self>, tail: Self, span: Span) -> Self {
        Self::new(
            FormKind::DottedList {
                items,
                tail: Box::new(tail),
            },
            span,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FormKind {
    Atom(String),
    String(String),
    Character(char),
    List(Vec<Form>),
    DottedList { items: Vec<Form>, tail: Box<Form> },
    Vector(Vec<Form>),
}

impl fmt::Display for Form {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FormKind::Atom(value) => formatter.write_str(value),
            FormKind::String(value) => write!(formatter, "{value:?}"),
            FormKind::Character(value) => write!(formatter, "#\\{value}"),
            FormKind::List(items) => {
                formatter.write_str("(")?;
                for (index, item) in items.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(" ")?;
                    }
                    item.fmt(formatter)?;
                }
                formatter.write_str(")")
            }
            FormKind::DottedList { items, tail } => {
                formatter.write_str("(")?;
                for item in items {
                    item.fmt(formatter)?;
                    formatter.write_str(" ")?;
                }
                formatter.write_str(". ")?;
                tail.fmt(formatter)?;
                formatter.write_str(")")
            }
            FormKind::Vector(items) => {
                formatter.write_str("#(")?;
                for (index, item) in items.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(" ")?;
                    }
                    item.fmt(formatter)?;
                }
                formatter.write_str(")")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Form, FormKind, Span};

    #[test]
    fn span_reports_length_and_empty_ranges() {
        assert_eq!(Span::new(4, 9).len(), 5);
        assert!(!Span::new(4, 9).is_empty());
        assert_eq!(Span::new(9, 4).len(), 0);
        assert!(Span::new(9, 4).is_empty());
    }

    #[test]
    fn form_constructors_preserve_kind_and_span() {
        let span = Span::new(1, 3);
        assert_eq!(
            Form::atom("name", span),
            Form::new(FormKind::Atom("name".into()), span)
        );
        assert_eq!(
            Form::list(vec![], span),
            Form::new(FormKind::List(vec![]), span)
        );
        assert_eq!(
            Form::dotted_list(vec![Form::atom("a", span)], Form::atom("b", span), span),
            Form::new(
                FormKind::DottedList {
                    items: vec![Form::atom("a", span)],
                    tail: Box::new(Form::atom("b", span)),
                },
                span,
            )
        );
    }

    #[test]
    fn display_covers_all_form_kinds() {
        let span = Span::new(0, 1);
        let cases = [
            (FormKind::Atom("name".into()), "name"),
            (FormKind::String("text".into()), "\"text\""),
            (FormKind::Character('x'), "#\\x"),
            (
                FormKind::List(vec![Form::atom("a", span), Form::atom("b", span)]),
                "(a b)",
            ),
            (
                FormKind::DottedList {
                    items: vec![Form::atom("a", span)],
                    tail: Box::new(Form::atom("b", span)),
                },
                "(a . b)",
            ),
            (FormKind::Vector(vec![Form::atom("a", span)]), "#(a)"),
        ];

        for (kind, expected) in cases {
            assert_eq!(Form::new(kind, span).to_string(), expected);
        }
    }
}
