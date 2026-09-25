//! Integer bit operations split by operation family.

use ncl_object::{BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word};

mod boole;
mod fields;
mod logic;

pub use boole::*;
use fields::{byte_position, byte_size, deposit_field, dpb, ldb, ldb_test, mask_field};
use logic::{
    ash, byte, integer, integer_length, integer_word, logand, logandc1, logandc2, logbitp,
    logcount, logeqv, logior, lognand, lognor, lognot, logorc1, logorc2, logtest, logxor,
};
