use std::fmt;
use std::fmt::Write as _;

use super::{
    ArrayElementType, Function, StructureRepresentation, Value, write_escaped_symbol,
    write_sequence,
};

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => formatter.write_str("NIL"),
            Self::Unbound => formatter.write_str("#<UNBOUND>"),
            Self::Boolean(true) => formatter.write_str("T"),
            Self::Boolean(false) => formatter.write_str("NIL"),
            Self::Integer(value) => value.fmt(formatter),
            Self::Rational(value) => write!(formatter, "{}/{}", value.numerator, value.denominator),
            Self::Float(value) => {
                if value.fract() == 0.0 {
                    write!(formatter, "{value:.1}")
                } else {
                    value.fmt(formatter)
                }
            }
            Self::Complex { real, imaginary } => write!(formatter, "#C({real} {imaginary})"),
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
            Self::Environment(_) => formatter.write_str("#<ENVIRONMENT>"),
            Self::Symbol(value) => formatter.write_str(value),
            Self::SymbolExact(value) => write_escaped_symbol(formatter, value),
            Self::QualifiedSymbolExact {
                reference,
                package_len,
            } => {
                formatter.write_str(&reference[..*package_len + 2])?;
                write_escaped_symbol(formatter, &reference[*package_len + 2..])
            }
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
                let values = values.borrow();
                write_sequence(formatter, &values)?;
                formatter.write_str(")")
            }
            Self::Array {
                dimensions,
                elements,
                element_type: ArrayElementType::Bit,
                ..
            } if dimensions.len() == 1 => {
                formatter.write_str("#*")?;
                for value in elements.borrow().iter() {
                    match value {
                        Self::Integer(0) => formatter.write_char('0')?,
                        Self::Integer(1) => formatter.write_char('1')?,
                        _ => return write!(formatter, "#<ARRAY {dimensions:?}>"),
                    }
                }
                Ok(())
            }
            Self::Array { dimensions, .. } => write!(formatter, "#<ARRAY {dimensions:?}>"),
            Self::HashTable { test, .. } => write!(formatter, "#<HASH-TABLE {test}>"),
            Self::HashTableIterator(_) => formatter.write_str("#<HASH-TABLE-ITERATOR>"),
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
            Self::Structure {
                name,
                slots,
                representation,
                named,
                marker,
                ..
            } => {
                if *representation == StructureRepresentation::Structure {
                    write!(formatter, "#S({name}")?;
                    for (slot_name, value) in slots.borrow().iter() {
                        write!(formatter, " :{slot_name} {value}")?;
                    }
                    return formatter.write_char(')');
                }
                let mut values = Vec::with_capacity(slots.borrow().len() + usize::from(*named));
                if *named {
                    values.push(
                        marker
                            .as_ref()
                            .map(|marker| marker.borrow().clone())
                            .unwrap_or_else(|| Value::symbol(name.as_ref())),
                    );
                }
                values.extend(slots.borrow().iter().map(|(_, value)| value.clone()));
                match representation {
                    StructureRepresentation::List => Value::list(values).fmt(formatter),
                    StructureRepresentation::Vector => Value::vector(values).fmt(formatter),
                    StructureRepresentation::Structure => unreachable!(),
                }
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
