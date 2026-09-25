#![allow(missing_docs)]

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    ArrayElementType, ArrayOptions, FindStatus, ObjectError, Package, Runtime, ThreadContext,
    WordView, array_dimensions, array_row_major_ref, array_row_major_set, bignum_limbs,
    bignum_sign, classify_object, complex_imag, complex_real, double_value, make_array,
    make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio, make_simple_vector,
    make_string, ratio_denominator, ratio_numerator, specialized_array_element_type,
    specialized_array_ref, specialized_array_set, symbol_name,
};
use ncl_sys::Word;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

#[path = "coverage_deep/arrays_and_errors.rs"]
mod arrays_and_errors;
#[path = "coverage_deep/hash_and_package.rs"]
mod hash_and_package;
