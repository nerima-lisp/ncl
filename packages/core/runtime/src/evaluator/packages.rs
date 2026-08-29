//! Package-system evaluator support: `DEFPACKAGE`, `IN-PACKAGE`, and the
//! form/value designator helpers shared by the package and symbol
//! primitives.
mod defpackage_eval;
mod defpackage_metadata_options;
mod defpackage_metadata_options_tests;
mod defpackage_parse;
mod defpackage_symbol_options;
mod defpackage_types;
mod designator_lookup;
mod designators_from_form;
mod designators_from_value;
mod in_package;
