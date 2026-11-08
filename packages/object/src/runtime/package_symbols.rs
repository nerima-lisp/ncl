use super::{RootedWord, Runtime};
use crate::{ObjectError, ThreadContext, Word};
use ncl_sys::StorageCondition;
use std::collections::HashMap;

#[derive(Debug)]
pub(super) struct PackageSymbols {
    package: RootedWord,
    internal: HashMap<String, RootedWord>,
    external: HashMap<String, RootedWord>,
}

impl Runtime {
    pub(crate) fn package_is_registered(&self, package: Word) -> bool {
        self.package_symbols
            .lock()
            .is_ok_and(|packages| packages.iter().any(|entry| entry.package.get() == package))
    }

    pub(crate) fn register_package_symbols(
        &self,
        ctx: &ThreadContext,
        package: Word,
    ) -> Result<(), ObjectError> {
        if self
            .package_symbols
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?
            .iter()
            .any(|entry| entry.package.get() == package)
        {
            return Ok(());
        }

        let mut symbols = PackageSymbols {
            package: RootedWord::new(&self.heap, package),
            internal: HashMap::new(),
            external: HashMap::new(),
        };
        for (slot, table) in [
            (crate::package::INTERNAL, &mut symbols.internal),
            (crate::package::EXTERNAL, &mut symbols.external),
        ] {
            for (name, symbol) in
                crate::package::symbol_table_entries(ctx, symbols.package.get(), slot)?
            {
                table.insert(name, RootedWord::new(&self.heap, symbol));
            }
        }

        let mut packages = self
            .package_symbols
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        if !packages
            .iter()
            .any(|entry| entry.package.get() == symbols.package.get())
        {
            packages.push(symbols);
        }
        drop(packages);
        Ok(())
    }

    pub(crate) fn package_symbol(
        &self,
        package: Word,
        external: bool,
        name: &str,
    ) -> Result<Option<Word>, ObjectError> {
        let packages = self
            .package_symbols
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        let symbol = packages
            .iter()
            .find(|entry| entry.package.get() == package)
            .and_then(|package| {
                let table = if external {
                    &package.external
                } else {
                    &package.internal
                };
                table.get(name).map(RootedWord::get)
            });
        drop(packages);
        Ok(symbol)
    }

    pub(crate) fn package_symbols_indexed(&self, package: Word) -> bool {
        self.package_symbols
            .lock()
            .is_ok_and(|symbols| symbols.iter().any(|entry| entry.package.get() == package))
    }

    pub(crate) fn insert_package_symbol(
        &self,
        package: Word,
        external: bool,
        name: String,
        symbol: Word,
    ) -> Result<(), ObjectError> {
        if !self.package_is_registered(package) {
            return Ok(());
        }
        let symbol = RootedWord::new(&self.heap, symbol);
        let mut packages = self
            .package_symbols
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        let package_symbols = packages
            .iter_mut()
            .find(|entry| entry.package.get() == package)
            .ok_or(ObjectError::Layout)?;
        let table = if external {
            &mut package_symbols.external
        } else {
            &mut package_symbols.internal
        };
        table.insert(name, symbol);
        drop(packages);
        Ok(())
    }

    pub(crate) fn remove_package_symbol(
        &self,
        package: Word,
        external: bool,
        name: &str,
    ) -> Result<(), ObjectError> {
        if !self.package_is_registered(package) {
            return Ok(());
        }
        let mut packages = self
            .package_symbols
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        let package_symbols = packages
            .iter_mut()
            .find(|entry| entry.package.get() == package)
            .ok_or(ObjectError::Layout)?;
        let table = if external {
            &mut package_symbols.external
        } else {
            &mut package_symbols.internal
        };
        table.remove(name);
        drop(packages);
        Ok(())
    }

    /// Release the runtime's indexed symbols for a package.
    ///
    /// # Errors
    /// Returns [`ObjectError::Storage`] if the package registry lock is unavailable.
    pub fn release_package_symbols(&self, package: Word) -> Result<(), ObjectError> {
        let mut packages = self
            .package_symbols
            .lock()
            .map_err(|_| ObjectError::Storage(StorageCondition::ThreadNotRegistered))?;
        packages.retain(|entry| entry.package.get() != package);
        drop(packages);
        Ok(())
    }
}
