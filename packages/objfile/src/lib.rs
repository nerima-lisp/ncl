//! Dependency-free writers and validators for NCL binary artifacts.
#![allow(missing_docs)]

mod elf;
mod error;
mod executable;
mod fasl;
mod macho;
#[cfg(test)]
mod tests;
mod types;

pub use elf::{
    sections_from_generic, validate_elf, ElfArchitecture, ElfObject, ElfReader, ElfSection,
    ElfSectionKind, ElfSymbol,
};
pub use error::ObjectError;
pub use executable::{write_elf_executable, write_mach_executable, ExecutableImage};
pub use fasl::{Architecture, Fasl, FaslHeader, FaslReader, FaslSection, FaslWriter};
pub use macho::{validate_macho, MachArchitecture, MachObject, MachReader, MachSection};
pub use types::{RelocKind, Relocation, Section, SectionId, SymbolRef};
