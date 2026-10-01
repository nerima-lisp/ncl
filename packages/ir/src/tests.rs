use crate::*;

#[test]
fn types_have_stable_display_and_primitive_effects() {
    assert_eq!(FunctionId(3).to_string(), "%3");
    assert_eq!(BlockId(4).to_string(), "%4");
    assert_eq!(ValueId(5).to_string(), "%5");
    assert_eq!(LocalId(6).to_string(), "%6");
    assert_eq!(ConstantIndex(7).to_string(), "%7");
    assert_eq!(FileId(8).to_string(), "%8");
    assert_eq!(FormId(9).to_string(), "%9");
    assert_eq!(HandlerRegionId(10).to_string(), "%10");
    assert_eq!(DebugLocationId(11).to_string(), "%11");

    let types = [Ty::Word, Ty::I64, Ty::F64, Ty::Address, Ty::Bool, Ty::Unit];
    let names = ["word", "i64", "f64", "address", "bool", "unit"];
    for (ty, name) in types.into_iter().zip(names) {
        assert_eq!(ty.to_string(), name);
    }

    for primitive in [Prim::Rplaca, Prim::Rplacd, Prim::Aset] {
        assert!(!primitive.is_pure());
        assert!(primitive.invalidates_memory());
        assert!(!primitive.reads_memory());
    }
    for primitive in [
        Prim::Car,
        Prim::Cdr,
        Prim::Svref,
        Prim::Aref,
        Prim::StructureSlot("slot".into()),
    ] {
        assert!(primitive.is_pure());
        assert!(primitive.reads_memory());
        assert!(!primitive.invalidates_memory());
    }
    assert!(Prim::FixnumAdd.is_pure());
    assert!(!Prim::FixnumAdd.reads_memory());
    assert!(!Prim::FixnumAdd.invalidates_memory());
}

#[test]
fn parse_error_displays_its_message() {
    let error = ParseError("bad integer".into());
    assert_eq!(error.to_string(), "bad integer");
    assert_eq!(error, ParseError("bad integer".into()));
}

#[path = "tests_handler.rs"]
mod tests_handler;
#[path = "tests_text.rs"]
mod tests_text;
#[path = "tests_verify.rs"]
mod tests_verify;
