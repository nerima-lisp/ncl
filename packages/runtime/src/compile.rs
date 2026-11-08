use std::path::Path;

use crate::{Runtime, RuntimeError};
use ncl_object::Word;

const FASL_FEATURES: u64 = 0;
const PAYLOAD_MAGIC: &[u8] = b"NCLRTFASL";
const PAYLOAD_VERSION: u16 = 1;

pub fn source(runtime: &mut Runtime, source: &str) -> Result<Word, RuntimeError> {
    runtime.eval(source)
}

pub fn file(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
    let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        error,
    })?;
    let contents = String::from_utf8(bytes)
        .map_err(|_| RuntimeError::Native("source file is not valid UTF-8".to_owned()))?;
    let value = crate::load::source_forms_with_mode(
        runtime,
        &contents,
        crate::evalwhen::TopLevelMode::CompileFile,
    )?;
    // This first FASL format deliberately stores source, so load recompiles it.
    // Native code, constants, and fixups are reserved for the persistent FASL format.
    let fasl = ncl_objfile::Fasl {
        header: ncl_objfile::FaslHeader {
            architecture: host_architecture(),
            features: FASL_FEATURES,
        },
        sections: ncl_objfile::FaslSection {
            code: Vec::new(),
            relocations: Vec::new(),
            constants: Vec::new(),
            symbols: Vec::new(),
            stack_maps: Vec::new(),
            debug: encode_payload(contents.as_bytes())?,
        },
    };
    let bytes = ncl_objfile::FaslWriter::write(&fasl)?;
    let output_path = path.with_extension("fasl");
    std::fs::write(&output_path, bytes).map_err(|error| RuntimeError::Io {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(value)
}

pub fn is_fasl(bytes: &[u8]) -> bool {
    bytes.get(..8) == Some(b"NCLFASL\0")
}

pub fn decode_payload(bytes: &[u8]) -> Result<String, RuntimeError> {
    let header_len = PAYLOAD_MAGIC.len() + 2 + 8 + 8;
    if bytes.len() < header_len || bytes.get(..PAYLOAD_MAGIC.len()) != Some(PAYLOAD_MAGIC) {
        return Err(RuntimeError::Native(
            "invalid runtime FASL payload magic".to_owned(),
        ));
    }
    let version_start = PAYLOAD_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes
            .get(version_start..version_start + 2)
            .ok_or_else(|| RuntimeError::Native("invalid runtime FASL payload version".to_owned()))?
            .try_into()
            .map_err(|_| RuntimeError::Native("invalid runtime FASL payload version".to_owned()))?,
    );
    if version != PAYLOAD_VERSION {
        return Err(RuntimeError::Native(
            "unsupported runtime FASL payload version".to_owned(),
        ));
    }
    let length_start = version_start + 2;
    let source_len = u64::from_le_bytes(
        bytes
            .get(length_start..length_start + 8)
            .ok_or_else(|| RuntimeError::Native("invalid runtime FASL source length".to_owned()))?
            .try_into()
            .map_err(|_| RuntimeError::Native("invalid runtime FASL source length".to_owned()))?,
    );
    let hash_start = length_start + 8;
    let expected_hash = u64::from_le_bytes(
        bytes
            .get(hash_start..hash_start + 8)
            .ok_or_else(|| RuntimeError::Native("invalid runtime FASL source hash".to_owned()))?
            .try_into()
            .map_err(|_| RuntimeError::Native("invalid runtime FASL source hash".to_owned()))?,
    );
    let source_start = header_len;
    let source_end = source_start
        .checked_add(
            usize::try_from(source_len)
                .map_err(|_| RuntimeError::Native("runtime FASL source is too large".to_owned()))?,
        )
        .ok_or_else(|| RuntimeError::Native("runtime FASL source length overflow".to_owned()))?;
    if source_end != bytes.len() {
        return Err(RuntimeError::Native(
            "invalid runtime FASL payload length".to_owned(),
        ));
    }
    let source = bytes
        .get(source_start..source_end)
        .ok_or_else(|| RuntimeError::Native("invalid runtime FASL source range".to_owned()))?;
    if fnv1a(source) != expected_hash {
        return Err(RuntimeError::Native(
            "runtime FASL source hash mismatch".to_owned(),
        ));
    }
    String::from_utf8(source.to_vec())
        .map_err(|_| RuntimeError::Native("runtime FASL source is not valid UTF-8".to_owned()))
}

fn encode_payload(source: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let source_len = u64::try_from(source.len())
        .map_err(|_| RuntimeError::Native("source is too large for runtime FASL".to_owned()))?;
    let mut payload =
        Vec::with_capacity(PAYLOAD_MAGIC.len() + 2 + std::mem::size_of::<u64>() * 2 + source.len());
    payload.extend_from_slice(PAYLOAD_MAGIC);
    payload.extend_from_slice(&PAYLOAD_VERSION.to_le_bytes());
    payload.extend_from_slice(&source_len.to_le_bytes());
    payload.extend_from_slice(&fnv1a(source).to_le_bytes());
    payload.extend_from_slice(source);
    Ok(payload)
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

const fn host_architecture() -> ncl_objfile::Architecture {
    if cfg!(target_arch = "x86_64") {
        ncl_objfile::Architecture::X86_64
    } else {
        ncl_objfile::Architecture::Aarch64
    }
}
