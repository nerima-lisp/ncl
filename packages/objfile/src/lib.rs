//! Dependency-free writers and validators for NCL binary artifacts.

mod elf;
mod error;
mod executable;
mod fasl;
mod fasl_code;
mod fasl_constants;
mod fasl_debug;
mod fasl_directory;
mod fasl_header;
mod fasl_legacy;
mod fasl_metadata;
mod fasl_records;
mod fasl_relocation;
mod fasl_stack_map;
mod fasl_symbols;
mod fasl_top_level;
mod fasl_validate;
mod macho;
#[cfg(test)]
mod tests;
mod types;

pub use elf::{
    ElfArchitecture, ElfObject, ElfReader, ElfSection, ElfSectionKind, ElfSymbol,
    sections_from_generic, validate_elf,
};
pub use error::{FaslError, FaslRejectReason, ObjectError};
pub use executable::{
    ExecutableImage, validate_elf_executable, write_elf_executable, write_mach_executable,
};
pub use fasl::{
    Architecture, Fasl, FaslHeader, FaslReader, FaslSection, FaslV1, FaslV1Compatibility,
    FaslV1Reader, FaslV1Writer, FaslWriter,
};
pub use fasl_code::FaslCodeObject;
pub use fasl_constants::{FaslConstant, FaslConstantGroup};
pub use fasl_debug::{DebugFile, DebugLocationKind, DebugRecord, DebugVariable};
pub use fasl_metadata::{
    FaslMetadata, FeatureRecord, PolicyRecord, compiler_identity, feature_fingerprint,
    policy_fingerprint, sha256,
};
pub use fasl_relocation::{FaslRelocation, FaslRelocationKind, FaslRelocationTarget};
pub use fasl_stack_map::{SafepointEntry, SafepointGroup};
pub use fasl_symbols::{FaslSymbol, SymbolBindingKind};
pub use fasl_top_level::FaslTopLevelRecord;
pub use macho::validate_mach_executable;
pub use macho::{MachArchitecture, MachObject, MachReader, MachSection, validate_macho};
pub use types::{RelocKind, Relocation, Section, SectionId, SymbolRef};
