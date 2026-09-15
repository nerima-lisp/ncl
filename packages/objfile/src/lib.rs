//! Dependency-free writers and validators for NCL binary artifacts.

mod elf;
mod error;
mod executable;
mod fasl;
mod macho;
#[cfg(test)]
mod tests;
mod types;

pub use elf::{
    ElfArchitecture, ElfObject, ElfReader, ElfSection, ElfSectionKind, ElfSymbol,
    sections_from_generic, validate_elf,
};
pub use error::ObjectError;
pub use executable::{
    ExecutableImage, validate_elf_executable, write_elf_executable, write_mach_executable,
};
pub use fasl::{Architecture, Fasl, FaslHeader, FaslReader, FaslSection, FaslWriter};
pub use macho::validate_mach_executable;
pub use macho::{MachArchitecture, MachObject, MachReader, MachSection, validate_macho};
pub use types::{RelocKind, Relocation, Section, SectionId, SymbolRef};
