//! Common Lisp package model operations.

mod lists;
mod package_operations;
mod runtime_operations;

use ncl_object::{ObjectError, Package, Runtime, StringObject, ThreadContext, Word};

/// CLHS operations over a package's internal and external symbol membership.
pub trait PackageListOperations {
    /// Add a package to this package's use list.
    fn use_package(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        package: Word,
    ) -> Result<bool, ObjectError>;

    /// Remove a package from this package's use list.
    fn unuse_package(self, ctx: &mut ThreadContext, package: Word) -> Result<bool, ObjectError>;

    /// Remove a symbol with the given name from this package.
    fn unintern(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError>;

    /// Ensure this package has a present symbol with the given name and marks it shadowing.
    fn shadow(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<(), ObjectError>;

    /// Import a symbol and mark it as shadowing in this package.
    fn shadowing_import(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        symbol: Word,
    ) -> Result<(), ObjectError>;

    /// Remove a package nickname.
    fn remove_nickname(self, ctx: &mut ThreadContext, nickname: Word) -> Result<bool, ObjectError>;
}

/// CLHS operations over package symbols, exports, and local nicknames.
pub trait PackageOperations {
    /// Visit each distinct internal and external symbol in the package.
    fn for_each_symbol<F>(self, ctx: &ThreadContext, visit: F) -> Result<(), ObjectError>
    where
        F: FnMut(Word);

    /// Add a nickname to this package.
    fn add_nickname(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        nickname: Word,
    ) -> Result<bool, ObjectError>;

    /// Export a present or inherited symbol from this package.
    fn export(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError>;

    /// Make an external symbol internal in this package.
    fn unexport(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
    ) -> Result<bool, ObjectError>;

    /// Import a symbol under the given name into this package.
    fn import(
        self,
        ctx: &mut ThreadContext,
        runtime: &Runtime,
        name: Word,
        symbol: Word,
    ) -> Result<(), ObjectError>;

    /// Resolve a package-local nickname.
    fn resolve_local_nickname(
        self,
        ctx: &ThreadContext,
        nickname: StringObject,
    ) -> Result<Option<Package>, ObjectError>;
}

pub use runtime_operations::RuntimePackageOperations;
