use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_code::{self, FaslCodeObject};
use crate::fasl_constants::{self, FaslConstantGroup};
use crate::fasl_debug::{self, DebugFile, DebugRecord};
use crate::fasl_directory::{self, Directory, SectionId};
use crate::fasl_header::{self, FaslV1Header};
use crate::fasl_metadata::{self, FaslMetadata};
use crate::fasl_records::FaslCodeLayout;
use crate::fasl_relocation::{self, FaslRelocation};
use crate::fasl_stack_map::{self, SafepointGroup};
use crate::fasl_symbols::{self, FaslSymbol};
use crate::fasl_top_level::{self, FaslTopLevelRecord};
use crate::fasl_validate;

pub use crate::fasl_legacy::{Architecture, Fasl, FaslHeader, FaslReader, FaslSection, FaslWriter};

/// Runtime values that must match before a native FASL v1 is accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaslV1Compatibility {
    /// Target architecture supported by the running runtime.
    pub architecture: Architecture,
    /// Exact target triple supported by the running runtime.
    pub target_triple: String,
    /// Runtime code and object ABI revision.
    pub runtime_abi_version: u32,
    /// Expected digest computed from compiler identity metadata.
    pub compiler_identity: [u8; 32],
    /// Expected digest of the runtime's canonical policy records.
    pub policy_fingerprint: [u8; 32],
    /// Expected digest of the runtime's required feature records.
    pub feature_fingerprint: [u8; 32],
}

/// A typed native FASL v1 compilation unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaslV1<'a> {
    /// Target architecture used to compile this unit.
    pub architecture: Architecture,
    /// Runtime code and object ABI revision.
    pub runtime_abi_version: u32,
    /// Compiler and source metadata used for compatibility checks.
    pub metadata: FaslMetadata,
    /// Symbol and native-import table.
    pub symbols: Vec<FaslSymbol>,
    /// Per-code-object constant vectors.
    pub constants: Vec<FaslConstantGroup>,
    /// Dense code objects with borrowed machine-code bytes.
    pub code: Vec<FaslCodeObject<'a>>,
    /// Typed code, import, and constant-index relocations.
    pub relocations: Vec<FaslRelocation>,
    /// Per-code-object safepoint maps.
    pub stack_maps: Vec<SafepointGroup>,
    /// Normalized source file table.
    pub debug_files: Vec<DebugFile>,
    /// Source locations and variable locations.
    pub debug_records: Vec<DebugRecord>,
    /// Ordered LOAD execution records.
    pub top_level: Vec<FaslTopLevelRecord>,
}

/// Stateless native FASL v1 serializer.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaslV1Writer;

impl FaslV1Writer {
    /// Encodes a typed native FASL v1 compilation unit.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a record, cross-reference, or section exceeds the wire contract.
    pub fn write(fasl: &FaslV1<'_>) -> Result<Vec<u8>, FaslError> {
        fasl_validate::validate_sections(fasl, None)?;

        let metadata = fasl_metadata::encode(&fasl.metadata)?;
        let symbols = fasl_symbols::encode(&fasl.symbols)?;
        let constants = fasl_constants::encode(&fasl.constants)?;
        let code = fasl_code::encode(&fasl.code)?;
        let relocations = fasl_relocation::encode_v1_section(&fasl.relocations)?;
        let stack_maps = fasl_stack_map::encode_section(&fasl.stack_maps)?;
        let debug = fasl_debug::encode_section(&fasl.debug_files, &fasl.debug_records)?;
        let top_level = fasl_top_level::encode(&fasl.top_level, &fasl.code)?;
        let sections = [
            metadata,
            symbols,
            constants,
            code,
            relocations,
            stack_maps,
            debug,
            top_level,
        ];
        let lengths = std::array::from_fn(|index| sections[index].len() as u64);
        let directory = Directory::from_lengths(lengths)?;
        let final_entry = directory.entries()[fasl_directory::SECTION_COUNT - 1];
        let file_length = (final_entry.file_offset + final_entry.byte_length) as usize;

        let header = FaslV1Header {
            architecture: fasl.architecture,
            runtime_abi_version: fasl.runtime_abi_version,
            compiler_identity: fasl_metadata::compiler_identity(&fasl.metadata)?,
            policy_fingerprint: fasl_metadata::policy_fingerprint(&fasl.metadata.policies)?,
            feature_fingerprint: fasl_metadata::feature_fingerprint(&fasl.metadata.features)?,
        };
        let mut bytes = vec![0; file_length];
        bytes[..fasl_header::HEADER_SIZE].copy_from_slice(&fasl_header::encode(&header));
        let directory_bytes = directory.encode();
        bytes[128..128 + directory_bytes.len()].copy_from_slice(&directory_bytes);
        for (entry, section) in directory.entries().iter().zip(&sections) {
            let start = entry.file_offset as usize;
            let end = start + section.len();
            bytes[start..end].copy_from_slice(section);
        }
        Ok(bytes)
    }
}

/// Stateless compatibility-checking native FASL v1 reader.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaslV1Reader;

impl FaslV1Reader {
    /// Validates a complete file and borrows its code bytes.
    ///
    /// Compatibility is checked after the metadata fingerprints have been recomputed.
    ///
    /// # Errors
    ///
    /// Returns a typed rejection for malformed, incompatible, or over-limit input.
    pub fn read<'a>(
        bytes: &'a [u8],
        expected: &FaslV1Compatibility,
    ) -> Result<FaslV1<'a>, FaslError> {
        validate_file_size(bytes.len())?;
        let header = fasl_header::decode(bytes)?;
        let directory_end = 128_usize + 8 * 24;
        let directory_bytes = bytes
            .get(128..directory_end)
            .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, 128))?;
        let directory = Directory::decode(directory_bytes)?;
        directory.validate_file(bytes)?;

        let (metadata_bytes, metadata_offset) = section(&directory, bytes, SectionId::Metadata)?;
        let metadata = fasl_metadata::decode(metadata_bytes, metadata_offset)?;
        validate_compatibility(&header, &metadata, expected)?;

        let (symbol_bytes, symbol_offset) = section(&directory, bytes, SectionId::Symbols)?;
        let symbols = fasl_symbols::decode(symbol_bytes, symbol_offset)?;
        let (constant_bytes, constant_offset) = section(&directory, bytes, SectionId::Constants)?;
        let constants = fasl_constants::decode(constant_bytes, constant_offset)?;
        let (code_bytes, code_offset) = section(&directory, bytes, SectionId::Code)?;
        let code = fasl_code::decode(code_bytes, code_offset)?;
        let layouts = code_layouts(&code);
        let (relocation_bytes, relocation_offset) =
            section(&directory, bytes, SectionId::Relocations)?;
        let relocations = fasl_relocation::decode_v1_section(relocation_bytes, relocation_offset)?;
        let (stack_map_bytes, stack_map_offset) = section(&directory, bytes, SectionId::StackMaps)?;
        let stack_maps =
            fasl_stack_map::decode_section(stack_map_bytes, stack_map_offset, &layouts)?;
        let (debug_bytes, debug_offset) = section(&directory, bytes, SectionId::DebugRecords)?;
        let (debug_files, debug_records) =
            fasl_debug::decode_section(debug_bytes, debug_offset, &layouts)?;
        let (top_level_bytes, top_level_offset) =
            section(&directory, bytes, SectionId::TopLevelRecords)?;
        let top_level = fasl_top_level::decode(top_level_bytes, top_level_offset, &code)?;
        let fasl = FaslV1 {
            architecture: header.architecture,
            runtime_abi_version: header.runtime_abi_version,
            metadata,
            symbols,
            constants,
            code,
            relocations,
            stack_maps,
            debug_files,
            debug_records,
            top_level,
        };
        fasl_validate::validate_sections(&fasl, Some(relocation_offset))?;
        Ok(fasl)
    }
}

fn validate_file_size(file_size: usize) -> Result<(), FaslError> {
    if file_size > fasl_directory::MAX_FILE_SIZE as usize {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(())
}

fn section<'a>(
    directory: &Directory,
    file: &'a [u8],
    id: SectionId,
) -> Result<(&'a [u8], usize), FaslError> {
    let entry = directory.entries()[id.index()];
    let offset = entry.file_offset as usize;
    Ok((directory.section(file, id)?, offset))
}

fn validate_compatibility(
    header: &FaslV1Header,
    metadata: &FaslMetadata,
    expected: &FaslV1Compatibility,
) -> Result<(), FaslError> {
    let compiler_identity = fasl_metadata::compiler_identity(metadata)?;
    if compiler_identity != header.compiler_identity {
        return Err(FaslError::at(
            FaslRejectReason::CompilerIdentityMismatch,
            24,
        ));
    }
    let policy_fingerprint = fasl_metadata::policy_fingerprint(&metadata.policies)?;
    if policy_fingerprint != header.policy_fingerprint {
        return Err(FaslError::at(
            FaslRejectReason::PolicyFingerprintMismatch,
            56,
        ));
    }
    let feature_fingerprint = fasl_metadata::feature_fingerprint(&metadata.features)?;
    if feature_fingerprint != header.feature_fingerprint {
        return Err(FaslError::at(
            FaslRejectReason::FeatureFingerprintMismatch,
            88,
        ));
    }
    if header.architecture != expected.architecture
        || metadata.target_triple != expected.target_triple
    {
        return Err(FaslError::at(FaslRejectReason::TargetMismatch, 10));
    }
    if header.runtime_abi_version != expected.runtime_abi_version {
        return Err(FaslError::at(FaslRejectReason::RuntimeAbiMismatch, 20));
    }
    if compiler_identity != expected.compiler_identity {
        return Err(FaslError::at(
            FaslRejectReason::CompilerIdentityMismatch,
            24,
        ));
    }
    if policy_fingerprint != expected.policy_fingerprint {
        return Err(FaslError::at(
            FaslRejectReason::PolicyFingerprintMismatch,
            56,
        ));
    }
    if feature_fingerprint != expected.feature_fingerprint {
        return Err(FaslError::at(
            FaslRejectReason::FeatureFingerprintMismatch,
            88,
        ));
    }
    Ok(())
}

fn code_layouts<'a>(code: &[FaslCodeObject<'a>]) -> Vec<FaslCodeLayout<'a>> {
    code.iter()
        .map(|object| FaslCodeLayout {
            id: object.id,
            entry_offset: object.entry_offset,
            frame_words: object.frame_words,
            constant_count: object.constant_count,
            code: object.code,
        })
        .collect()
}

#[cfg(test)]
#[path = "tests_fasl_v1.rs"]
mod private_tests;
