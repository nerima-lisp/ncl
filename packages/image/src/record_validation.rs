use crate::error::ImageError;

use super::record_refs::Ref;
use super::record_types::Record;

/// Validate all references against an already decoded object-table length.
pub fn validate_record(record: &Record, count: usize) -> Result<(), ImageError> {
    let check = |reference: Ref| validate_ref(reference, count);
    match record {
        Record::Cons { car, cdr } => {
            check(*car)?;
            check(*cdr)?;
        }
        Record::Symbol {
            value,
            function,
            plist,
            ..
        } => {
            check(*value)?;
            check(*function)?;
            check(*plist)?;
        }
        Record::Vector(values) | Record::Structure { slots: values } => {
            for value in values {
                check(*value)?;
            }
        }
        Record::HashTable { entries, .. } => {
            for (key, value) in entries {
                check(*key)?;
                check(*value)?;
            }
        }
        Record::Instance { class, slots } => {
            check(*class)?;
            for value in slots {
                check(*value)?;
            }
        }
        Record::Function {
            name,
            lambda_list,
            code,
            captures,
            ..
        } => {
            check(*name)?;
            check(*lambda_list)?;
            check(*code)?;
            for value in captures {
                check(*value)?;
            }
        }
        Record::CodeObject {
            constants,
            stack_map,
            debug,
            ..
        } => {
            check(*constants)?;
            check(*stack_map)?;
            check(*debug)?;
        }
        Record::Ratio {
            numerator,
            denominator,
        } => {
            check(*numerator)?;
            check(*denominator)?;
        }
        Record::Complex { real, imag } => {
            check(*real)?;
            check(*imag)?;
        }
        Record::Package { .. }
        | Record::String(_)
        | Record::SpecializedArray { .. }
        | Record::Bignum { .. }
        | Record::DoubleFloat { .. } => {}
    }
    Ok(())
}

/// Validate one reference against an object-table length.
pub fn validate_ref(reference: Ref, count: usize) -> Result<(), ImageError> {
    match reference {
        Ref::Immediate(_) => Ok(()),
        Ref::Object(index) if (index as usize) < count => Ok(()),
        Ref::Object(_) => Err(ImageError::InvalidLayout {
            field: "object reference",
        }),
    }
}
