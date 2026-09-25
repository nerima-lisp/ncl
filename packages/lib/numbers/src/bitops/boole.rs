use super::{
    ash, byte, byte_position, byte_size, deposit_field, dpb, integer, integer_length, integer_word,
    ldb, ldb_test, logand, logandc1, logandc2, logbitp, logcount, logeqv, logior, lognand, lognor,
    lognot, logorc1, logorc2, logtest, logxor, mask_field, BuiltinArgs, MultipleValues,
    ObjectError, Runtime, ThreadContext, Word,
};

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

macro_rules! typed_dispatch {
    ($name:ident, $legacy:ident) => {
        pub fn $name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            $legacy(runtime, ctx, args.as_slice(), values)
        }
    };
}

typed_dispatch!(typed_logand, logand);
typed_dispatch!(typed_logior, logior);
typed_dispatch!(typed_logxor, logxor);
typed_dispatch!(typed_lognot, lognot);
typed_dispatch!(typed_logeqv, logeqv);
typed_dispatch!(typed_lognand, lognand);
typed_dispatch!(typed_lognor, lognor);
typed_dispatch!(typed_logandc1, logandc1);
typed_dispatch!(typed_logandc2, logandc2);
typed_dispatch!(typed_logorc1, logorc1);
typed_dispatch!(typed_logorc2, logorc2);
typed_dispatch!(typed_logtest, logtest);
typed_dispatch!(typed_logbitp, logbitp);
typed_dispatch!(typed_logcount, logcount);
typed_dispatch!(typed_integer_length, integer_length);
typed_dispatch!(typed_ash, ash);
typed_dispatch!(typed_byte, byte);
typed_dispatch!(typed_byte_size, byte_size);
typed_dispatch!(typed_byte_position, byte_position);
typed_dispatch!(typed_ldb, ldb);
typed_dispatch!(typed_dpb, dpb);
typed_dispatch!(typed_ldb_test, ldb_test);
typed_dispatch!(typed_mask_field, mask_field);
typed_dispatch!(typed_deposit_field, deposit_field);
typed_dispatch!(typed_boole, boole);
