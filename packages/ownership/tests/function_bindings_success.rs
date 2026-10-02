#![allow(missing_docs, clippy::unwrap_used)]

use ncl_object::{Runtime, ThreadContext};
use ncl_ownership::assert_crate_function_bindings_from_table;

#[test]
fn function_binding_check_accepts_registered_and_bound_function_cells() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    runtime.register_keyword_builtins(&mut ctx).unwrap();
    let table = "package\tsymbol\tkind\tcrate\tphase\tdirect-expansion\tnotes\nNCL-EXT\tMAKE-REST-LIST\tfunction\ttest\t1\tno\t\n";

    assert!(assert_crate_function_bindings_from_table(&runtime, &mut ctx, table, "test").is_ok());
}
