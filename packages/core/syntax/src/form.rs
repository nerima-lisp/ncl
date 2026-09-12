mod display;

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
#[derive(Clone, Debug)]
pub struct Form {
    /// The parsed syntax node.
    pub kind: FormKind,
    /// The node's source location.
    pub span: Span,
    /// Quotation returns this object instead of reconstructing `kind`.
    /// Clear it when rewriting syntax to represent a different object.
    /// Equality compares `kind` and `span`, ignoring this annotation.
    pub original_value: Option<crate::OpaqueLiteral>,
}

impl PartialEq for Form {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.span == other.span
    }
}

impl Form {
    /// Creates a form with the supplied kind and location.
    #[must_use]
    pub const fn new(kind: FormKind, span: Span) -> Self {
        Self {
            kind,
            span,
            original_value: None,
        }
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

/// Reader syntax and retained literals introduced during compilation.
#[derive(Clone, Debug, PartialEq)]
pub enum FormKind {
    /// A projection containing a cycle; quotation recovers its annotated object.
    CircularReference,
    /// A retained runtime literal, not constructible by the reader.
    Literal(crate::OpaqueLiteral),
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
    /// A complex number literal with real and imaginary components.
    Complex {
        /// The real component.
        real: Box<Form>,
        /// The imaginary component.
        imaginary: Box<Form>,
    },
}

#[cfg(test)]
mod tests {
    use super::{Form, FormKind, Span};

    #[test]
    fn annotation_clone_preserves_original_allocation() {
        let original = crate::OpaqueLiteral::new(String::from("original"));
        let mut form = Form::atom("x", Span::new(0, 1));
        form.original_value = Some(original.clone());
        let cloned = form.clone();
        assert_eq!(form.original_value, cloned.original_value);
        assert_eq!(cloned.original_value, Some(original));
    }

    #[test]
    fn annotation_is_absent_on_new_and_rebuilt_forms() {
        let mut form = Form::new(FormKind::Atom("x".into()), Span::new(0, 1));
        assert!(form.original_value.is_none());
        form.original_value = Some(crate::OpaqueLiteral::new(1_i64));
        let rebuilt = Form::new(form.kind.clone(), form.span);
        assert!(rebuilt.original_value.is_none());
    }

    #[test]
    fn annotation_does_not_affect_structural_equality() {
        let mut left = Form::atom("x", Span::new(0, 1));
        let mut right = left.clone();
        left.original_value = Some(crate::OpaqueLiteral::new(1_i64));
        right.original_value = Some(crate::OpaqueLiteral::new(2_i64));
        assert_ne!(left.original_value, right.original_value);
        assert_eq!(left, right);
        assert_eq!(left, Form::atom("x", left.span));
        assert_ne!(left, Form::atom("y", left.span));
        assert_ne!(left, Form::atom("x", Span::new(1, 2)));
    }

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
            (
                FormKind::Complex {
                    real: Box::new(Form::atom("1", span)),
                    imaginary: Box::new(Form::atom("2", span)),
                },
                "#C(1 2)",
            ),
        ];
        for (kind, expected) in cases {
            assert_eq!(Form::new(kind, span).to_string(), expected);
        }
    }
}
