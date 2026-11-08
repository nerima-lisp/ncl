//! Symbol and binding identities remain scoped to their declared owners.

#![deny(
    unreachable_pub,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::as_conversions
)]

use std::error::Error;

use ncl_hir::binding::{BindingEnvironment, BindingError, BindingUseKind};
use ncl_hir::ids::{ArenaIndex, CompilationUnitId, FormId, NodeId, SymbolId};
use ncl_hir::{SymbolError, SymbolTable};

#[test]
fn uninterned_symbol_identity_is_form_local_and_name_consistent() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(53);
    let first_form = FormId::new(unit, 0);
    let other_form = FormId::new(unit, 1);
    let foreign_form = FormId::new(CompilationUnitId::new(54), 0);
    let mut symbols = SymbolTable::new(unit);

    let first = symbols.intern(first_form, None, "TEMP", Some(7))?;
    assert_eq!(symbols.intern(first_form, None, "TEMP", Some(7)), Ok(first));
    let same_name_new_identity = symbols.intern(first_form, None, "TEMP", Some(8))?;
    let same_identity_other_form = symbols.intern(other_form, None, "TEMP", Some(7))?;

    assert_ne!(first, same_name_new_identity);
    assert_ne!(first, same_identity_other_form);
    assert_eq!(first.index(), 0);
    assert_eq!(same_name_new_identity.index(), 1);
    assert_eq!(same_identity_other_form.index(), 2);
    assert_eq!(
        symbols.get(first).map(|symbol| (
            symbol.package(),
            symbol.name(),
            symbol.uninterned_identity()
        )),
        Some((None, "TEMP", Some(7)))
    );
    assert_eq!(
        symbols.intern(first_form, None, "RENAMED", Some(7)),
        Err(SymbolError::ConflictingUninternedName {
            form: first_form,
            identity: 7,
        })
    );
    assert_eq!(
        symbols.intern(first_form, None, "MISSING-IDENTITY", None),
        Err(SymbolError::InvalidIdentityShape)
    );
    assert_eq!(
        symbols.intern(first_form, Some("CL"), "HAS-BOTH-IDENTITIES", Some(9)),
        Err(SymbolError::InvalidIdentityShape)
    );
    assert_eq!(
        symbols.intern(foreign_form, Some("CL"), "FOREIGN", None),
        Err(SymbolError::ForeignForm(foreign_form))
    );

    let foreign_symbol = SymbolId::try_from_index(foreign_form.compilation_unit(), 0)?;
    assert_eq!(symbols.get(foreign_symbol), None);
    assert_eq!(symbols.ids().len(), 3);
    assert!(symbols.ids().contains(&first));
    assert!(symbols.ids().contains(&same_name_new_identity));
    assert!(symbols.ids().contains(&same_identity_other_form));
    Ok(())
}

#[test]
fn binding_scopes_and_bindings_reject_foreign_form_owners() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(55);
    let form = FormId::new(unit, 0);
    let foreign_form = FormId::new(unit, 1);
    let symbol = SymbolId::try_from_index(unit, 0)?;
    let foreign_symbol = SymbolId::try_from_index(unit, 1)?;
    let mut bindings = BindingEnvironment::new(form);
    let root = bindings.root_scope();
    let local_binding = bindings.bind_variable(root, symbol)?;
    let child = bindings.push_scope(root)?;
    assert_eq!(bindings.scope_parent(root), Ok(None));
    assert_eq!(bindings.scope_parent(child), Ok(Some(root)));

    let mut foreign_bindings = BindingEnvironment::new(foreign_form);
    let foreign_scope = foreign_bindings.root_scope();
    let foreign_binding = foreign_bindings.bind_variable(foreign_scope, foreign_symbol)?;
    let mut other_bindings = BindingEnvironment::new(form);
    let other_root = other_bindings.root_scope();
    let other_child = other_bindings.push_scope(other_root)?;
    let unallocated_scope = other_bindings.push_scope(other_child)?;
    assert_eq!(foreign_scope.index(), root.index());
    assert_eq!(local_binding.index(), foreign_binding.index());
    assert_eq!(foreign_binding.form(), foreign_form);
    assert_ne!(foreign_scope, root);
    assert_ne!(foreign_binding, local_binding);
    for invalid_scope in [foreign_scope, unallocated_scope] {
        assert_eq!(
            bindings.scope_parent(invalid_scope),
            Err(BindingError::UnknownScope(invalid_scope))
        );
        assert_eq!(
            bindings.push_scope(invalid_scope),
            Err(BindingError::UnknownScope(invalid_scope))
        );
        assert_eq!(
            bindings.bind_variable(invalid_scope, symbol),
            Err(BindingError::UnknownScope(invalid_scope))
        );
        assert_eq!(
            bindings.bind_function(invalid_scope, symbol),
            Err(BindingError::UnknownScope(invalid_scope))
        );
        assert_eq!(
            bindings.bind_local_functions(
                invalid_scope,
                ncl_hir::binding::LocalFunctionKind::Flet,
                &[]
            ),
            Err(BindingError::UnknownScope(invalid_scope))
        );
        assert_eq!(
            bindings.resolve_variable(invalid_scope, symbol),
            Err(BindingError::UnknownScope(invalid_scope))
        );
        assert_eq!(
            bindings.resolve_function(invalid_scope, symbol),
            Err(BindingError::UnknownScope(invalid_scope))
        );
    }
    assert_eq!(bindings.binding(foreign_binding), None);
    assert_eq!(
        bindings.record_use(
            foreign_binding,
            NodeId::try_from_index(form, 0)?,
            BindingUseKind::Read
        ),
        Err(BindingError::UnknownBinding(foreign_binding))
    );
    Ok(())
}

#[test]
fn checked_form_ids_preserve_owner_when_indexes_match() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::try_from_index(56)?;
    let foreign_unit = CompilationUnitId::try_from_index(57)?;
    let form = <FormId as ArenaIndex>::try_from_index(unit, 4)?;
    let same_index_foreign_form = FormId::try_from_index(foreign_unit, 4)?;

    assert_eq!(ArenaIndex::owner(form), unit);
    assert_eq!(ArenaIndex::index(form), 4);
    assert_eq!(form.compilation_unit(), unit);
    assert_eq!(form.index(), same_index_foreign_form.index());
    assert_ne!(form, same_index_foreign_form);
    Ok(())
}

#[test]
fn symbol_errors_display_their_context() {
    assert_eq!(
        SymbolError::InvalidIdentityShape.to_string(),
        "symbol table error: InvalidIdentityShape"
    );
}
