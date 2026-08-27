use std::fmt;

/// A half-open byte range in the source text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

impl Span {
    /// Creates a source span.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the span length, saturating when the bounds are reversed.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns whether the span contains no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// A parsed Lisp form and its source location.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    /// The parsed syntax node.
    pub kind: FormKind,
    /// The node's source location.
    pub span: Span,
}

impl Form {
    /// Creates a form with the supplied kind and location.
    #[must_use]
    pub const fn new(kind: FormKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Creates an atom form.
    #[must_use]
    pub fn atom(value: impl Into<String>, span: Span) -> Self {
        Self::new(FormKind::Atom(value.into()), span)
    }

    /// Creates a proper list form.
    #[must_use]
    pub const fn list(items: Vec<Self>, span: Span) -> Self {
        Self::new(FormKind::List(items), span)
    }

    /// Creates an improper (dotted) list form.
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

/// The syntactic variants accepted by the reader.
#[derive(Clone, Debug, PartialEq)]
pub enum FormKind {
    /// An unparsed atom token.
    Atom(String),
    /// A string literal.
    String(String),
    /// A character literal.
    Character(char),
    /// A proper list.
    List(Vec<Form>),
    /// An improper list with a distinct tail.
    DottedList {
        /// Forms before the dot.
        items: Vec<Form>,
        /// Form after the dot.
        tail: Box<Form>,
    },
    /// A vector literal.
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
    fn span_and_form_constructors_preserve_data() {
        let span = Span::new(3, 8);
        assert_eq!(span.len(), 5);
        assert!(!span.is_empty());
        assert_eq!(Span::new(8, 3).len(), 0);
        assert!(Span::new(8, 8).is_empty());

        let atom = Form::atom("x", span);
        assert_eq!(atom, Form::new(FormKind::Atom("x".into()), span));
        assert_eq!(Form::list(vec![atom.clone()], span).to_string(), "(x)");
        assert_eq!(
            Form::dotted_list(vec![atom.clone()], atom, span).to_string(),
            "(x . x)"
        );
    }

    #[test]
    fn every_form_variant_has_a_human_readable_display() {
        let span = Span::new(0, 1);
        let cases = [
            (FormKind::Atom("x".into()), "x"),
            (FormKind::String("x".into()), "\"x\""),
            (FormKind::Character('x'), "#\\x"),
            (FormKind::List(vec![]), "()"),
            (FormKind::List(vec![Form::atom("x", span)]), "(x)"),
            (
                FormKind::List(vec![Form::atom("x", span), Form::atom("y", span)]),
                "(x y)",
            ),
            (FormKind::Vector(vec![]), "#()"),
            (
                FormKind::Vector(vec![
                    Form::atom("x", span),
                    Form::new(FormKind::String("y".into()), span),
                ]),
                "#(x \"y\")",
            ),
        ];
        for (kind, expected) in cases {
            assert_eq!(Form::new(kind, span).to_string(), expected);
        }
    }
}
