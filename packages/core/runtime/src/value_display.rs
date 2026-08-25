use std::fmt::{self, Write as _};

use super::*;

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => formatter.write_str("NIL"),
            Self::Unbound => formatter.write_str("#<UNBOUND>"),
            Self::Boolean(true) => formatter.write_str("T"),
            Self::Boolean(false) => formatter.write_str("NIL"),
            Self::Integer(value) => value.fmt(formatter),
            Self::Rational(value) => {
                write!(formatter, "{}/{}", value.numerator(), value.denominator())
            }
            Self::Float(value) => {
                if value.fract() == 0.0 {
                    write!(formatter, "{value:.1}")
                } else {
                    value.fmt(formatter)
                }
            }
            Self::String(value) => write!(formatter, "{value:?}"),
            Self::Character(value) => match value {
                ' ' => formatter.write_str("#\\SPACE"),
                '\n' => formatter.write_str("#\\NEWLINE"),
                '\t' => formatter.write_str("#\\TAB"),
                '\r' => formatter.write_str("#\\RETURN"),
                value => write!(formatter, "#\\{value}"),
            },
            Self::Stream(stream) => write!(formatter, "#<{}>", stream.borrow().kind_name()),
            Self::Package(value) => write!(formatter, "#<PACKAGE \"{value}\">"),
            Self::Environment(_) => formatter.write_str("#<ENVIRONMENT>"),
            Self::Symbol(value) => formatter.write_str(value),
            Self::SymbolExact(value) => write_escaped_symbol(formatter, value),
            Self::UninternedSymbol(value) => write!(formatter, "#:{value}"),
            Self::Keyword(value) => write!(formatter, ":{value}"),
            Self::KeywordExact(value) => {
                formatter.write_char(':')?;
                write_escaped_symbol(formatter, value)
            }
            Self::List(values) => {
                formatter.write_str("(")?;
                write_sequence(formatter, values)?;
                formatter.write_str(")")
            }
            Self::DottedList { items, tail } => {
                formatter.write_str("(")?;
                write_sequence(formatter, items)?;
                if !items.is_empty() {
                    formatter.write_str(" ")?;
                }
                write!(formatter, ". {tail})")
            }
            Self::Vector(values) => {
                formatter.write_str("#(")?;
                write_sequence(formatter, values)?;
                formatter.write_str(")")
            }
            Self::Array { dimensions, .. } => write!(formatter, "#<ARRAY {dimensions:?}>"),
            Self::HashTable { test, .. } => write!(formatter, "#<HASH-TABLE {test}>"),
            Self::Values(values) => {
                formatter.write_str("#<VALUES")?;
                if !values.is_empty() {
                    formatter.write_str(" ")?;
                    write_sequence(formatter, values)?;
                }
                formatter.write_str(">")
            }
            Self::Condition(condition) => write!(formatter, "#<CONDITION {}>", condition.message),
            Self::Restart(restart) => write!(formatter, "#<RESTART {}>", restart.name),
            Self::Structure { name, slots, .. } => {
                write!(formatter, "#S({name}")?;
                for (slot_name, value) in slots.borrow().iter() {
                    write!(formatter, " :{slot_name} {value}")?;
                }
                formatter.write_char(')')
            }
            Self::Class(definition) => write!(formatter, "#<CLASS {}>", definition.name),
            Self::Instance(instance) => write!(formatter, "#<{} INSTANCE>", instance.class.name),
            Self::Function(function) => match function.as_ref() {
                Function::Builtin { name, .. } => write!(formatter, "#<BUILTIN {name}>"),
                Function::Primitive { name } => write!(formatter, "#<PRIMITIVE {name}>"),
                Function::StructureConstructor { name, .. } => {
                    write!(formatter, "#<STRUCTURE-CONSTRUCTOR {name}>")
                }
                Function::StructurePredicate { name } => {
                    write!(formatter, "#<STRUCTURE-PREDICATE {name}>")
                }
                Function::StructureAccessor {
                    structure_name,
                    slot_name,
                    ..
                } => write!(
                    formatter,
                    "#<STRUCTURE-ACCESSOR {structure_name}-{slot_name}>"
                ),
                Function::StructureCopier { name } => {
                    write!(formatter, "#<STRUCTURE-COPIER {name}>")
                }
                Function::Generic { name, .. } => write!(formatter, "#<GENERIC-FUNCTION {name}>"),
                Function::SlotReader {
                    class_name,
                    slot_name,
                } => write!(formatter, "#<SLOT-READER {class_name}-{slot_name}>"),
                Function::SlotWriter {
                    class_name,
                    slot_name,
                } => write!(formatter, "#<SLOT-WRITER {class_name}-{slot_name}>"),
                Function::ConditionReader {
                    condition_name,
                    slot_name,
                } => write!(
                    formatter,
                    "#<CONDITION-READER {condition_name}-{slot_name}>"
                ),
                Function::ConditionWriter {
                    condition_name,
                    slot_name,
                } => write!(
                    formatter,
                    "#<CONDITION-WRITER {condition_name}-{slot_name}>"
                ),
                Function::Closure { .. } | Function::Compiled { .. } => {
                    formatter.write_str("#<FUNCTION>")
                }
                Function::Macro { .. } | Function::ModifyMacro { .. } => {
                    formatter.write_str("#<MACRO>")
                }
            },
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Value(")?;
        fmt::Display::fmt(self, formatter)?;
        formatter.write_str(")")
    }
}

fn write_sequence(formatter: &mut fmt::Formatter<'_>, values: &[Value]) -> fmt::Result {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            formatter.write_str(" ")?;
        }
        fmt::Display::fmt(value, formatter)?;
    }
    Ok(())
}

fn write_escaped_symbol(formatter: &mut fmt::Formatter<'_>, value: &str) -> fmt::Result {
    formatter.write_char('|')?;
    for character in value.chars() {
        if matches!(character, '|' | '\\') {
            formatter.write_char('\\')?;
        }
        formatter.write_char(character)?;
    }
    formatter.write_char('|')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display_cases() -> Vec<(Value, &'static str)> {
        vec![
            (
                Value::Rational(Rational::new(3, 4).expect("valid rational")),
                "3/4",
            ),
            (
                Value::DottedList {
                    items: Rc::new(Vec::new()),
                    tail: Rc::new(Value::Nil),
                },
                "(. NIL)",
            ),
            (
                Value::array(vec![2, 3], vec![Value::Nil; 6]),
                "#<ARRAY [2, 3]>",
            ),
            (Value::hash_table("EQUAL"), "#<HASH-TABLE EQUAL>"),
            (
                Value::condition_from_parts(
                    "ERROR".to_owned(),
                    "failed".to_owned(),
                    None,
                    Vec::new(),
                ),
                "#<CONDITION failed>",
            ),
            (Value::restart("retry"), "#<RESTART retry>"),
            (Value::generic("dispatch"), "#<GENERIC-FUNCTION dispatch>"),
            (
                Value::slot_reader("person", "name"),
                "#<SLOT-READER person-name>",
            ),
            (
                Value::slot_writer("person", "name"),
                "#<SLOT-WRITER person-name>",
            ),
            (
                Value::closure(Vec::new(), Vec::new(), Environment::new()),
                "#<FUNCTION>",
            ),
        ]
    }

    #[test]
    fn displays_internal_value_variants_through_one_table() {
        for (value, expected) in display_cases() {
            assert_eq!(value.to_string(), expected, "displaying {:?}", value);
            assert_eq!(format!("{:?}", value), format!("Value({expected})"));
        }
    }

    #[test]
    fn displays_every_function_shape_with_a_stable_name() {
        let cases = [
            (
                Value::Function(Rc::new(Function::StructurePredicate {
                    name: "person-p".to_owned(),
                })),
                "#<STRUCTURE-PREDICATE person-p>",
            ),
            (
                Value::Function(Rc::new(Function::StructureAccessor {
                    structure_name: "person".to_owned(),
                    slot_name: "name".to_owned(),
                    slot_index: 0,
                    read_only: true,
                })),
                "#<STRUCTURE-ACCESSOR person-name>",
            ),
            (
                Value::Function(Rc::new(Function::StructureCopier {
                    name: "copy-person".to_owned(),
                })),
                "#<STRUCTURE-COPIER copy-person>",
            ),
            (
                Value::Function(Rc::new(Function::ConditionReader {
                    condition_name: "error".to_owned(),
                    slot_name: "message".to_owned(),
                })),
                "#<CONDITION-READER error-message>",
            ),
            (
                Value::Function(Rc::new(Function::ConditionWriter {
                    condition_name: "error".to_owned(),
                    slot_name: "message".to_owned(),
                })),
                "#<CONDITION-WRITER error-message>",
            ),
        ];

        for (value, expected) in cases {
            assert_eq!(value.to_string(), expected);
        }
    }
}
