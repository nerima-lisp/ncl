//! Common Lisp hash-table model.

#![forbid(unsafe_code)]

mod hash_table;

pub use hash_table::{HashTable, HashTest, Weakness, register_layouts, sxhash};
