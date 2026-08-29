#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) const ARRAY_HASH_BUILTINS: &[BuiltinDefinition] = &[
    ("vector", vector as _),
    ("make-array", make_array as _),
    ("make-sequence", make_sequence as _),
    ("aref", aref as _),
    ("svref", svref as _),
    ("bit", bit as _),
    ("row-major-aref", row_major_aref as _),
    ("array-row-major-index", array_row_major_index as _),
    ("array-in-bounds-p", array_in_bounds_p as _),
    ("array-element-type", array_element_type as _),
    ("simple-array-p", simple_array_p as _),
    ("arrayp", arrayp as _),
    ("array-rank", array_rank as _),
    ("array-dimensions", array_dimensions as _),
    ("array-dimension", array_dimension as _),
    ("array-total-size", array_total_size as _),
    ("make-hash-table", make_hash_table as _),
    ("gethash", gethash as _),
    ("remhash", remhash as _),
    ("clrhash", clrhash as _),
    ("hash-table-p", hash_table_p as _),
    ("hash-table-count", hash_table_count as _),
    ("hash-table-test", hash_table_test_value as _),
];
