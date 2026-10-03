use crate::{ObjectError, ThreadContext, Word};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct StructureRegistry {
    pub by_layout: HashMap<u32, StructureDescription>,
    pub by_symbol: HashMap<StructureSymbol, u32>,
}

#[derive(Debug)]
pub struct StructureDescription {
    pub layout: crate::StructureLayout,
    pub parent: Option<crate::StructureLayout>,
    pub name: StructureSymbol,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StructureSymbol {
    package: String,
    pub name: String,
}

impl StructureSymbol {
    pub fn from_word(ctx: &ThreadContext, symbol: Word) -> Result<Self, ObjectError> {
        let name = symbol_text(ctx, crate::symbol_name(ctx, symbol)?)?;
        let package = crate::Package::from_word(crate::symbol_package(ctx, symbol)?).name(ctx)?;
        Ok(Self {
            package: symbol_text(ctx, package)?,
            name,
        })
    }

    pub fn qualified_name(&self) -> String {
        format!("{}::{}", self.package, self.name)
    }
}

impl StructureRegistry {
    pub fn register(&mut self, symbol: StructureSymbol, description: StructureDescription) {
        let layout = description.layout.into();
        self.by_symbol.insert(symbol, layout);
        self.by_layout.insert(layout, description);
    }
}

fn symbol_text(ctx: &ThreadContext, string: Word) -> Result<String, ObjectError> {
    let length = crate::string_length(ctx, string)?;
    (0..length)
        .map(|index| crate::string_ref(ctx, string, index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(package: &str, name: &str) -> StructureSymbol {
        StructureSymbol {
            package: package.to_owned(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn register_indexes_layout_and_qualified_symbol_values() {
        let name = symbol("NCL-TEST", "POINT");
        let layout = crate::StructureLayout::from(17);
        let description = StructureDescription {
            layout,
            parent: None,
            name: name.clone(),
        };
        let mut registry = StructureRegistry::default();

        registry.register(name.clone(), description);

        assert_eq!(registry.by_symbol.get(&name), Some(&17));
        let stored = registry
            .by_layout
            .get(&17)
            .unwrap_or_else(|| panic!("layout missing"));
        assert_eq!(stored.layout, layout);
        assert_eq!(stored.parent, None);
        assert_eq!(stored.name.qualified_name(), "NCL-TEST::POINT");
    }

    #[test]
    fn duplicate_layout_replaces_description_but_keeps_symbol_index_value() {
        let first = symbol("NCL-TEST", "FIRST");
        let second = symbol("NCL-TEST", "SECOND");
        let layout = crate::StructureLayout::from(3);
        let mut registry = StructureRegistry::default();

        registry.register(
            first.clone(),
            StructureDescription {
                layout,
                parent: None,
                name: first.clone(),
            },
        );
        registry.register(
            second.clone(),
            StructureDescription {
                layout,
                parent: Some(crate::StructureLayout::from(2)),
                name: second.clone(),
            },
        );

        assert_eq!(registry.by_symbol.get(&first), Some(&3));
        assert_eq!(registry.by_symbol.get(&second), Some(&3));
        assert_eq!(
            registry.by_layout.get(&3).map(|value| value.parent),
            Some(Some(2.into()))
        );
        assert_eq!(registry.by_layout.len(), 1);
    }
}
