//! Common Lisp BOOLE operation codes and the BOOLE callback.

use ncl_object::{MultipleValues, ObjectError, Runtime, ThreadContext, Word};

use super::{integer, integer_word};

pub const BOOLE_CLR: i64 = 0;
pub const BOOLE_1: i64 = 10;
pub const BOOLE_2: i64 = 12;
pub const BOOLE_C1: i64 = 5;
pub const BOOLE_C2: i64 = 3;
pub const BOOLE_AND: i64 = 8;
pub const BOOLE_IOR: i64 = 14;
pub const BOOLE_XOR: i64 = 6;
pub const BOOLE_EQV: i64 = 9;
pub const BOOLE_NAND: i64 = 7;
pub const BOOLE_NOR: i64 = 1;
pub const BOOLE_ANDC1: i64 = 4;
pub const BOOLE_ANDC2: i64 = 2;
pub const BOOLE_ORC1: i64 = 13;
pub const BOOLE_ORC2: i64 = 11;
pub const BOOLE_SET: i64 = 15;

pub fn boole(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let [opcode, a, b] = args else {
        return Err(ObjectError::TypeError);
    };
    let opcode = integer(ctx, *opcode)?;
    let a = integer(ctx, *a)?;
    let b = integer(ctx, *b)?;
    let result = match opcode {
        0 => 0,
        1 => !(a | b),
        2 => !a & !b,
        3 => !a,
        4 => a & !b,
        5 => !b,
        6 => a ^ b,
        7 => !(a & b),
        8 => a & b,
        9 => !(a ^ b),
        10 => a,
        11 => a | !b,
        12 => b,
        13 => !a | b,
        14 => a | b,
        15 => -1,
        _ => return Err(ObjectError::TypeError),
    };
    integer_word(ctx, runtime, result)
}
