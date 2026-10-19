#![allow(missing_docs)]
#![allow(clippy::expect_used)]

use ncl_runtime::Runtime;

#[test]
fn assoc_accepts_nil_as_an_empty_alist() {
    let mut runtime = Runtime::new().expect("runtime");
    let result = runtime
        .eval("(assoc :missing nil)")
        .expect("assoc evaluation");
    assert_eq!(runtime.format_result(result), "NIL");
}
