use super::*;
use crate::{Package, Runtime, make_string, make_symbol};

#[test]
fn structure_symbol_preserves_qualified_names_and_rejects_bad_words() -> Result<(), ObjectError> {
    let runtime = Runtime::new()?;
    let mut context = ThreadContext::new();
    context.register(&runtime)?;
    let package = Package::new(&mut context, &runtime, "STRUCTURE-REGISTRY")?;
    let (symbol, _) = package.intern(&mut context, &runtime, "POINT")?;
    let name = StructureSymbol::from_word(&context, symbol)?;
    assert_eq!(name.package, "STRUCTURE-REGISTRY");
    assert_eq!(name.name, "POINT");
    assert_eq!(name.qualified_name(), "STRUCTURE-REGISTRY::POINT");
    let invalid_name = make_symbol(&mut context, &runtime, Word::fixnum(1))?;
    assert_eq!(
        StructureSymbol::from_word(&context, invalid_name),
        Err(ObjectError::TypeError)
    );
    let string = make_string(&mut context, &runtime, &['N'])?;
    let uninterned = make_symbol(&mut context, &runtime, string)?;
    assert_eq!(
        StructureSymbol::from_word(&context, uninterned),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn structure_registry_registers_both_lookup_directions() {
    let layout = crate::StructureLayout::from(7);
    let parent = crate::StructureLayout::from(3);
    let symbol = StructureSymbol {
        package: "STRUCTURE-REGISTRY".to_owned(),
        name: "POINT".to_owned(),
    };
    let mut registry = StructureRegistry::default();
    registry.register(
        symbol.clone(),
        StructureDescription {
            layout,
            parent: Some(parent),
            name: symbol.clone(),
        },
    );
    assert_eq!(registry.by_symbol.get(&symbol), Some(&7));
    let description = registry
        .by_layout
        .get(&7)
        .unwrap_or_else(|| panic!("registered layout"));
    assert_eq!(description.layout, layout);
    assert_eq!(description.parent, Some(parent));
    assert_eq!(description.name, symbol);
}
