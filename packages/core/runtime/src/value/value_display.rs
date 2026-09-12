use std::fmt::{self, Write};

use super::{Function, Value};

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil | Self::Boolean(false) => formatter.write_str("NIL"),
            Self::Unbound => formatter.write_str("#<UNBOUND>"),
            Self::Boolean(true) => formatter.write_str("T"),
            Self::Integer(value) => value.fmt(formatter),
            Self::BigInteger(value) => value.fmt(formatter),
            Self::Rational(value) => {
                write!(formatter, "{}/{}", value.numerator(), value.denominator())
            }
            Self::BigRational(value) => {
                write!(formatter, "{}/{}", value.numerator(), value.denominator())
            }
            Self::Float(value) => {
                if value.fract() == 0.0 {
                    write!(formatter, "{value:.1}")
                } else {
                    value.fmt(formatter)
                }
            }
            Self::Complex(value) => write!(formatter, "#C({} {})", value.real(), value.imaginary()),
            Self::String(value) => write!(formatter, "{value:?}"),
            Self::Character(value) => match value {
                ' ' => formatter.write_str("#\\SPACE"),
                '\n' => formatter.write_str("#\\NEWLINE"),
                '\t' => formatter.write_str("#\\TAB"),
                '\r' => formatter.write_str("#\\RETURN"),
                value => write!(formatter, "#\\{value}"),
            },
            Self::Stream(stream) => write!(formatter, "#<{}>", stream.borrow().kind_name()),
            Self::RandomState(_) => formatter.write_str("#<RANDOM-STATE>"),
            Self::Package(value) => write!(formatter, "#<PACKAGE \"{value}\">"),
            Self::PackageObject(value) => match value.name() {
                Some(name) => write!(formatter, "#<PACKAGE \"{name}\">"),
                None => formatter.write_str("#<PACKAGE (deleted)>"),
            },
            Self::Environment(_) => formatter.write_str("#<ENVIRONMENT>"),
            Self::Symbol(value) => formatter.write_str(value),
            Self::SymbolExact(value) => write_escaped_symbol(formatter, value),
            Self::InternedSymbol(value) if value.exact() && value.keyword() => {
                formatter.write_char(':')?;
                write_escaped_symbol(formatter, value.name())
            }
            Self::InternedSymbol(value) => formatter.write_str(&value.reference()),
            Self::UninternedSymbol(value) => write!(formatter, "#:{value}"),
            Self::Keyword(value) => write!(formatter, ":{value}"),
            Self::KeywordExact(value) => {
                formatter.write_char(':')?;
                write_escaped_symbol(formatter, value)
            }
            Self::Cons(cell) => formatter.write_str(&cell.printed_with(ToString::to_string)),
            Self::Vector(values) => {
                let Some(_guard) =
                    super::PrintGuard::enter(crate::value::PrintKind::Vector, values.identity())
                else {
                    return formatter.write_str("#<CIRCULAR>");
                };
                formatter.write_str("#(")?;
                write_sequence(formatter, &values.visible_snapshot())?;
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
                let Some(_guard) = super::PrintGuard::enter(
                    crate::value::PrintKind::Structure,
                    std::rc::Rc::as_ptr(slots) as usize,
                ) else {
                    return formatter.write_str("#<CIRCULAR>");
                };
                write!(formatter, "#S({name}")?;
                let snapshot = slots.borrow().clone();
                for (slot_name, value) in &snapshot {
                    write!(formatter, " :{slot_name} {value}")?;
                }
                formatter.write_char(')')
            }
            Self::Class(definition) => write!(formatter, "#<CLASS {}>", definition.name),
            Self::Instance(instance) => {
                write!(formatter, "#<{} INSTANCE>", instance.class.borrow().name)
            }
            Self::Function(function) => fmt_function(formatter, function),
        }
    }
}

fn fmt_function(formatter: &mut fmt::Formatter<'_>, function: &Function) -> fmt::Result {
    match function {
        Function::Complement { .. }
        | Function::Constantly { .. }
        | Function::Method { .. }
        | Function::SlotSetfWriter { .. } => formatter.write_str("#<FUNCTION>"),
        Function::Builtin { name, .. } => write!(formatter, "#<BUILTIN {name}>"),
        Function::Primitive { name } => write!(formatter, "#<PRIMITIVE {name}>"),
        Function::StructureConstructor { name, .. } => {
            write!(formatter, "#<STRUCTURE-CONSTRUCTOR {name}>")
        }
        Function::StructurePredicate { name } => write!(formatter, "#<STRUCTURE-PREDICATE {name}>"),
        Function::StructureAccessor {
            structure_name,
            slot_name,
            ..
        } => write!(
            formatter,
            "#<STRUCTURE-ACCESSOR {structure_name}-{slot_name}>"
        ),
        Function::StructureCopier { name } => write!(formatter, "#<STRUCTURE-COPIER {name}>"),
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
        Function::Closure { .. }
        | Function::HashTableIterator { .. }
        | Function::Compiled { .. } => formatter.write_str("#<FUNCTION>"),
        Function::Macro { .. } | Function::ModifyMacro { .. } => formatter.write_str("#<MACRO>"),
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
