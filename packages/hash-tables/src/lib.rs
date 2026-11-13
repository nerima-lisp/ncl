//! Common Lisp hash-table model.

#![forbid(unsafe_code)]

mod hash_table;

pub use hash_table::{HashTable, HashTest, Weakness, new_with_options, register_layouts, sxhash};
