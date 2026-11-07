//! Tests for typed arena identity, append-only indexes, bounds, and overflow.

#![deny(
    unreachable_pub,
    unused_crate_dependencies,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]

#[path = "../src/arena.rs"]
/// The arena implementation under test.
pub mod arena;
#[path = "../src/ids.rs"]
/// The typed identities used by the arena tests.
pub mod ids;

use arena::Arena;
use ids::{CompilationUnitId, FormId, IdError, NodeId, SymbolId};

#[test]
fn local_ids_reject_other_forms_and_out_of_bounds_positions() {
    let compilation_unit = CompilationUnitId::new(0);
    let owner = FormId::new(compilation_unit, 0);
    let other_form = FormId::new(compilation_unit, 1);
    let other_unit_form = FormId::new(CompilationUnitId::new(1), 0);
    let mut arena = Arena::<u32, NodeId>::new(owner);
    let mut other_arena = Arena::<u32, NodeId>::new(other_form);
    let mut other_unit_arena = Arena::<u32, NodeId>::new(other_unit_form);
    let id = arena.allocate(17);
    let _other_id = other_arena.allocate(23);
    let _other_unit_id = other_unit_arena.allocate(29);

    assert_eq!(id.map(NodeId::index), Ok(0));
    assert_eq!(
        id.ok().and_then(|value| arena.get(value)).copied(),
        Some(17)
    );
    assert_eq!(
        id.ok().and_then(|value| other_arena.get(value)),
        None,
        "an equal numeric index from another form is not owned here"
    );
    assert_eq!(
        id.ok().and_then(|value| other_unit_arena.get(value)),
        None,
        "an equal form index from another compilation unit is not owned here"
    );
    assert_eq!(
        NodeId::try_from_index(owner, 1).map(|value| arena.get(value)),
        Ok(None),
        "an in-scope index outside the arena length is rejected"
    );
}

#[test]
fn appending_preserves_existing_ids_without_reusing_positions() {
    let owner = FormId::new(CompilationUnitId::new(2), 3);
    let mut arena = Arena::<u32, NodeId>::new(owner);
    let first = arena.allocate(10);
    let second = arena.allocate(20);
    let third = arena.allocate(30);

    assert_eq!(first.map(NodeId::index), Ok(0));
    assert_eq!(second.map(NodeId::index), Ok(1));
    assert_eq!(third.map(NodeId::index), Ok(2));
    assert_eq!(first.ok().and_then(|id| arena.get(id)).copied(), Some(10));
    assert_eq!(second.ok().and_then(|id| arena.get(id)).copied(), Some(20));
    assert_eq!(arena.len(), 3);
}

#[test]
fn symbol_identity_is_compilation_unit_scoped_not_form_scoped() {
    let unit = CompilationUnitId::new(4);
    let mut symbols = Arena::<u32, SymbolId>::new(unit);
    let symbol = symbols.allocate(99);

    assert_eq!(
        symbol.map(SymbolId::compilation_unit),
        Ok(unit),
        "symbols carry their compilation unit as owner"
    );
    assert_eq!(
        symbol.ok().and_then(|id| symbols.get(id)).copied(),
        Some(99)
    );

    let other_unit = CompilationUnitId::new(5);
    let other_symbols = Arena::<u32, SymbolId>::new(other_unit);
    assert_eq!(
        symbol.ok().and_then(|id| other_symbols.get(id)),
        None,
        "equal symbol indexes from different compilation units are distinct"
    );
}

#[cfg(target_pointer_width = "64")]
#[test]
fn ids_reject_indexes_that_overflow_u32() {
    let unit = CompilationUnitId::new(0);
    let form = FormId::new(unit, 0);
    let too_large = u64::from(u32::MAX)
        .checked_add(1)
        .and_then(|index| usize::try_from(index).ok());

    assert!(too_large.is_some());
    if let Some(index) = too_large {
        assert_eq!(
            NodeId::try_from_index(form, index),
            Err(IdError::IndexOverflow { index })
        );
        assert_eq!(
            FormId::try_from_index(unit, index),
            Err(IdError::IndexOverflow { index })
        );
        assert_eq!(
            CompilationUnitId::try_from_index(index),
            Err(IdError::IndexOverflow { index })
        );
    }
}
