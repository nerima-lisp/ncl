use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    pub kind: FormKind,
    pub span: Span,
}

impl Form {
    pub const fn new(kind: FormKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub fn atom(value: impl Into<String>, span: Span) -> Self {
        Self::new(FormKind::Atom(value.into()), span)
    }

    pub fn list(items: Vec<Self>, span: Span) -> Self {
        Self::new(FormKind::List(items), span)
    }

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
