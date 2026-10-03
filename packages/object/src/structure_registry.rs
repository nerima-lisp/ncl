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
#[path = "../tests/support/structure_registry_tests.rs"]
mod tests;
