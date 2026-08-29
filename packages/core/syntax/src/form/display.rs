use std::fmt;

use crate::{Form, FormKind};

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
