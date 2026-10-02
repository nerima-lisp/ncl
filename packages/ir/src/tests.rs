use crate::*;

#[test]
fn types_have_stable_display_and_primitive_effects() {
    assert_eq!(FunctionId(3).to_string(), "%3"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(BlockId(4).to_string(), "%4"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(ValueId(5).to_string(), "%5"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(LocalId(6).to_string(), "%6"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(ConstantIndex(7).to_string(), "%7"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(FileId(8).to_string(), "%8"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(FormId(9).to_string(), "%9"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(HandlerRegionId(10).to_string(), "%10"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(DebugLocationId(11).to_string(), "%11"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions

    let types = [Ty::Word, Ty::I64, Ty::F64, Ty::Address, Ty::Bool, Ty::Unit];
    let names = ["word", "i64", "f64", "address", "bool", "unit"];
    for (ty, name) in types.into_iter().zip(names) {
        assert_eq!(ty.to_string(), name); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    }

    for primitive in [Prim::Rplaca, Prim::Rplacd, Prim::Aset] {
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        assert!(!primitive.is_pure()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        assert!(primitive.invalidates_memory()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        assert!(!primitive.reads_memory()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    }
    for primitive in [
        Prim::Car,
        Prim::Cdr,
        Prim::Svref,
        Prim::Aref,
        Prim::StructureSlot("slot".into()),
    ] {
        assert!(primitive.is_pure()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        assert!(primitive.reads_memory()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
        assert!(!primitive.invalidates_memory()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    }
    assert!(Prim::FixnumAdd.is_pure()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert!(!Prim::FixnumAdd.reads_memory()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert!(!Prim::FixnumAdd.invalidates_memory()); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
}

#[test]
fn parse_error_displays_its_message() {
    let error = ParseError("bad integer".into());
    assert_eq!(error.to_string(), "bad integer"); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
    assert_eq!(error, ParseError("bad integer".into())); // check-added-lines: allow(panic,index,as-cast) test fixture assertions
}

#[path = "tests_builder.rs"]
mod tests_builder;
#[path = "tests_handler.rs"]
mod tests_handler;
#[path = "tests_text.rs"]
mod tests_text;
#[path = "tests_text_coverage.rs"]
mod tests_text_coverage;
#[path = "tests_text_edges.rs"]
mod tests_text_edges;
#[path = "tests_verify.rs"]
mod tests_verify;
#[path = "tests_verify_coverage.rs"]
mod tests_verify_coverage;
