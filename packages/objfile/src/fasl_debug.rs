use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_records::{FaslCodeLayout, SectionReader, SectionWriter};

const MAX_RECORDS: usize = 1_048_576;
const MAX_STRING_BYTES: usize = 1_048_576;

/// One normalized source path in the debug section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugFile {
    /// Dense file ID referenced by debug records.
    pub file_id: u32,
    /// Normalized repository-relative UTF-8 path.
    pub path: String,
}

/// A source variable's location in a native frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugLocationKind {
    /// Location is a slot in the native frame.
    StackSlot,
    /// Location is a saved register.
    SavedRegister,
}

/// One named variable location in a debug record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugVariable {
    /// Canonical UTF-8 variable name.
    pub name: String,
    /// Kind of native location.
    pub location_kind: DebugLocationKind,
    /// Frame slot or saved-register ID.
    pub location_index: u32,
}

/// A code range and its source-level variable locations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugRecord {
    /// Dense code-object identifier.
    pub code_object_id: u32,
    /// Inclusive code-relative PC start.
    pub pc_start: u32,
    /// Exclusive code-relative PC end.
    pub pc_end: u32,
    /// Dense source-file ID.
    pub file_id: u32,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
    /// Inclusive source-form byte start.
    pub source_start: u32,
    /// Exclusive source-form byte end.
    pub source_end: u32,
    /// Variables in unique name-byte order.
    pub variables: Vec<DebugVariable>,
}

/// Decodes debug files and records and validates references against code objects.
pub(super) fn decode_section(
    bytes: &[u8],
    base_offset: usize,
    code_objects: &[FaslCodeLayout<'_>],
) -> Result<(Vec<DebugFile>, Vec<DebugRecord>), FaslError> {
    let mut reader = SectionReader::new(bytes, base_offset, FaslRejectReason::InvalidDebugRecord);
    let file_count_offset = reader.absolute_position();
    let file_count = bounded_count(reader.u32()?, file_count_offset)?;
    if file_count > reader.remaining() / 12 {
        return Err(FaslError::at(
            FaslRejectReason::InvalidDebugRecord,
            file_count_offset,
        ));
    }
    let mut files = Vec::with_capacity(file_count);
    for expected_id in 0..file_count {
        let record_offset = reader.absolute_position();
        let file_id = reader.u32()?;
        if usize::try_from(file_id).ok() != Some(expected_id) {
            return Err(FaslError::at(
                FaslRejectReason::InvalidDebugRecord,
                record_offset,
            ));
        }
        let length_offset = reader.absolute_position();
        let length = bounded_string_length(reader.u32()?, length_offset)?;
        if length == 0 {
            return Err(FaslError::at(
                FaslRejectReason::InvalidDebugRecord,
                length_offset,
            ));
        }
        let path_offset = reader.absolute_position();
        let path = core::str::from_utf8(reader.take(length)?)
            .map_err(|_| FaslError::at(FaslRejectReason::InvalidDebugRecord, path_offset))?
            .to_owned();
        if !is_normalized_relative_path(&path)
            || files
                .last()
                .is_some_and(|previous: &DebugFile| previous.path.as_bytes() >= path.as_bytes())
        {
            return Err(FaslError::at(
                FaslRejectReason::InvalidDebugRecord,
                path_offset,
            ));
        }
        reader.zero_padding(4)?;
        files.push(DebugFile { file_id, path });
    }
    let record_count_offset = reader.absolute_position();
    let record_count = bounded_count(reader.u32()?, record_count_offset)?;
    if record_count > reader.remaining() / 36 {
        return Err(FaslError::at(
            FaslRejectReason::InvalidDebugRecord,
            record_count_offset,
        ));
    }
    let mut records = Vec::with_capacity(record_count);
    let mut previous_key = None;
    for _ in 0..record_count {
        let record_offset = reader.absolute_position();
        let code_object_id = reader.u32()?;
        let pc_start = reader.u32()?;
        let pc_end = reader.u32()?;
        let file_id = reader.u32()?;
        let line = reader.u32()?;
        let column = reader.u32()?;
        let source_start = reader.u32()?;
        let source_end = reader.u32()?;
        let variable_count_offset = reader.absolute_position();
        let variable_count = bounded_count(reader.u32()?, variable_count_offset)?;
        if variable_count > reader.remaining() / 16 {
            return Err(FaslError::at(
                FaslRejectReason::InvalidDebugRecord,
                variable_count_offset,
            ));
        }
        let key = (code_object_id, pc_start);
        if previous_key.is_some_and(|previous| key <= previous) {
            return Err(FaslError::at(
                FaslRejectReason::InvalidDebugRecord,
                record_offset,
            ));
        }
        let code = code_layout(code_objects, code_object_id, record_offset)?;
        let code_length = u32::try_from(code.code.len())
            .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, record_offset))?;
        if pc_start >= pc_end
            || pc_end > code_length
            || usize::try_from(file_id)
                .ok()
                .is_none_or(|id| id >= files.len())
            || line == 0
            || column == 0
            || source_start > source_end
        {
            return Err(FaslError::at(
                FaslRejectReason::InvalidDebugRecord,
                record_offset,
            ));
        }
        let mut variables = Vec::with_capacity(variable_count);
        for _ in 0..variable_count {
            let variable_offset = reader.absolute_position();
            let name_length_offset = reader.absolute_position();
            let name_length = bounded_string_length(reader.u32()?, name_length_offset)?;
            if name_length == 0 {
                return Err(FaslError::at(
                    FaslRejectReason::InvalidDebugRecord,
                    name_length_offset,
                ));
            }
            let name_offset = reader.absolute_position();
            let name = core::str::from_utf8(reader.take(name_length)?)
                .map_err(|_| FaslError::at(FaslRejectReason::InvalidDebugRecord, name_offset))?
                .to_owned();
            let location_kind_offset = reader.absolute_position();
            let location_kind = match reader.u32()? {
                0 => DebugLocationKind::StackSlot,
                1 => DebugLocationKind::SavedRegister,
                _ => {
                    return Err(FaslError::at(
                        FaslRejectReason::InvalidDebugRecord,
                        location_kind_offset,
                    ));
                }
            };
            let location_index = reader.u32()?;
            if name.as_bytes().contains(&0)
                || variables.last().is_some_and(|previous: &DebugVariable| {
                    previous.name.as_bytes() >= name.as_bytes()
                })
                || !valid_location(location_kind, location_index, code.frame_words)
            {
                return Err(FaslError::at(
                    FaslRejectReason::InvalidDebugRecord,
                    variable_offset,
                ));
            }
            reader.zero_padding(4)?;
            variables.push(DebugVariable {
                name,
                location_kind,
                location_index,
            });
        }
        records.push(DebugRecord {
            code_object_id,
            pc_start,
            pc_end,
            file_id,
            line,
            column,
            source_start,
            source_end,
            variables,
        });
        previous_key = Some(key);
    }
    reader.finish()?;
    Ok((files, records))
}

/// Encodes debug files and records in their canonical order.
pub(super) fn encode_section(
    files: &[DebugFile],
    records: &[DebugRecord],
) -> Result<Vec<u8>, FaslError> {
    if files.len() > MAX_RECORDS || records.len() > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    if files
        .windows(2)
        .any(|pair| pair[0].path.as_bytes() >= pair[1].path.as_bytes())
    {
        return Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
    }
    let mut writer = SectionWriter::new();
    writer.put_u32(count_u32(files.len())?);
    for (expected_id, file) in files.iter().enumerate() {
        if usize::try_from(file.file_id).ok() != Some(expected_id)
            || file.path.is_empty()
            || !is_normalized_relative_path(&file.path)
            || file.path.len() > MAX_STRING_BYTES
        {
            return Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
        }
        writer.put_u32(file.file_id);
        writer.put_u32(count_u32(file.path.len())?);
        writer.put_bytes(file.path.as_bytes());
        writer.zero_padding(4);
    }
    let mut ordered_records = records.to_vec();
    ordered_records.sort_by_key(|record| (record.code_object_id, record.pc_start));
    if ordered_records.windows(2).any(|pair| {
        (pair[0].code_object_id, pair[0].pc_start) == (pair[1].code_object_id, pair[1].pc_start)
    }) {
        return Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
    }
    writer.put_u32(count_u32(ordered_records.len())?);
    for mut record in ordered_records {
        if record.variables.len() > MAX_RECORDS {
            return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
        }
        record
            .variables
            .sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
        if record
            .variables
            .windows(2)
            .any(|pair| pair[0].name == pair[1].name)
            || usize::try_from(record.file_id)
                .ok()
                .is_none_or(|id| id >= files.len())
            || record.pc_start >= record.pc_end
            || record.line == 0
            || record.column == 0
            || record.source_start > record.source_end
        {
            return Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
        }
        writer.put_u32(record.code_object_id);
        writer.put_u32(record.pc_start);
        writer.put_u32(record.pc_end);
        writer.put_u32(record.file_id);
        writer.put_u32(record.line);
        writer.put_u32(record.column);
        writer.put_u32(record.source_start);
        writer.put_u32(record.source_end);
        writer.put_u32(count_u32(record.variables.len())?);
        for variable in record.variables {
            if variable.name.is_empty()
                || variable.name.len() > MAX_STRING_BYTES
                || variable.name.as_bytes().contains(&0)
            {
                return Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
            }
            writer.put_u32(count_u32(variable.name.len())?);
            writer.put_bytes(variable.name.as_bytes());
            writer.put_u32(match variable.location_kind {
                DebugLocationKind::StackSlot => 0,
                DebugLocationKind::SavedRegister => 1,
            });
            writer.put_u32(variable.location_index);
            writer.zero_padding(4);
        }
    }
    Ok(writer.into_bytes())
}

/// Checks debug ranges and variable locations against decoded code objects.
pub(super) fn validate(
    files: &[DebugFile],
    records: &[DebugRecord],
    code_objects: &[FaslCodeLayout<'_>],
) -> Result<(), FaslError> {
    let mut previous_path: Option<&[u8]> = None;
    for (expected_id, file) in files.iter().enumerate() {
        if usize::try_from(file.file_id).ok() != Some(expected_id)
            || !is_normalized_relative_path(&file.path)
            || file.path.len() > MAX_STRING_BYTES
            || previous_path.is_some_and(|previous| previous >= file.path.as_bytes())
        {
            return Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
        }
        previous_path = Some(file.path.as_bytes());
    }
    let mut previous_key = None;
    for record in records {
        let key = (record.code_object_id, record.pc_start);
        let code = code_layout(code_objects, record.code_object_id, 0)?;
        let code_length = u32::try_from(code.code.len())
            .map_err(|_| FaslError::new(FaslRejectReason::InvalidDebugRecord))?;
        if previous_key.is_some_and(|previous| key <= previous)
            || record.pc_start >= record.pc_end
            || record.pc_end > code_length
            || usize::try_from(record.file_id)
                .ok()
                .is_none_or(|id| id >= files.len())
            || record.line == 0
            || record.column == 0
            || record.source_start > record.source_end
        {
            return Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
        }
        let mut previous_name: Option<&[u8]> = None;
        for variable in &record.variables {
            if variable.name.is_empty()
                || variable.name.len() > MAX_STRING_BYTES
                || variable.name.as_bytes().contains(&0)
                || previous_name.is_some_and(|previous| previous >= variable.name.as_bytes())
                || !valid_location(
                    variable.location_kind,
                    variable.location_index,
                    code.frame_words,
                )
            {
                return Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
            }
            previous_name = Some(variable.name.as_bytes());
        }
        previous_key = Some(key);
    }
    Ok(())
}

fn is_normalized_relative_path(path: &str) -> bool {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.as_bytes().contains(&0)
        || path.as_bytes().get(1) == Some(&b':')
    {
        return false;
    }
    path.split('/')
        .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn valid_location(kind: DebugLocationKind, index: u32, frame_words: u16) -> bool {
    match kind {
        DebugLocationKind::StackSlot => index < u32::from(frame_words),
        DebugLocationKind::SavedRegister => index < 16,
    }
}

fn code_layout<'a, 'b>(
    code_objects: &'a [FaslCodeLayout<'b>],
    id: u32,
    offset: usize,
) -> Result<&'a FaslCodeLayout<'b>, FaslError> {
    let index = usize::try_from(id)
        .map_err(|_| FaslError::at(FaslRejectReason::InvalidDebugRecord, offset))?;
    code_objects
        .get(index)
        .filter(|layout| layout.id == id)
        .ok_or_else(|| FaslError::at(FaslRejectReason::InvalidReference, offset))
}

fn count_u32(count: usize) -> Result<u32, FaslError> {
    u32::try_from(count).map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))
}

fn bounded_count(count: u32, offset: usize) -> Result<usize, FaslError> {
    let count = usize::try_from(count)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, offset))?;
    if count > MAX_RECORDS {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            offset,
        ));
    }
    Ok(count)
}

fn bounded_string_length(length: u32, offset: usize) -> Result<usize, FaslError> {
    let length = usize::try_from(length)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, offset))?;
    if length > MAX_STRING_BYTES {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            offset,
        ));
    }
    Ok(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_path_and_variable_record_round_trip_exact_bytes() {
        let files = [DebugFile {
            file_id: 0,
            path: "a.lisp".into(),
        }];
        let records = [DebugRecord {
            code_object_id: 0,
            pc_start: 0,
            pc_end: 4,
            file_id: 0,
            line: 1,
            column: 1,
            source_start: 0,
            source_end: 2,
            variables: vec![DebugVariable {
                name: "x".into(),
                location_kind: DebugLocationKind::StackSlot,
                location_index: 2,
            }],
        }];
        let expected = vec![
            1, 0, 0, 0, // file count
            0, 0, 0, 0, // file ID
            6, 0, 0, 0, // path length
            b'a', b'.', b'l', b'i', b's', b'p', 0, 0, // path and padding
            1, 0, 0, 0, // record count
            0, 0, 0, 0, // code-object ID
            0, 0, 0, 0, // PC start
            4, 0, 0, 0, // PC end
            0, 0, 0, 0, // file ID
            1, 0, 0, 0, // line
            1, 0, 0, 0, // column
            0, 0, 0, 0, // source start
            2, 0, 0, 0, // source end
            1, 0, 0, 0, // variable count
            1, 0, 0, 0, b'x', // variable name
            0, 0, 0, 0, // stack slot location kind
            2, 0, 0, 0, // stack slot index
            0, 0, 0, // padding
        ];
        let encoded = encode_section(&files, &records);
        assert_eq!(encoded, Ok(expected.clone()));
        let layout = [FaslCodeLayout {
            id: 0,
            entry_offset: 0,
            frame_words: 4,
            constant_count: 0,
            code: &[0; 4],
        }];
        assert_eq!(
            decode_section(&expected, 0, &layout),
            Ok((files.to_vec(), records.to_vec()))
        );
    }

    #[test]
    fn debug_rejects_parent_path_and_out_of_frame_location() {
        assert!(!is_normalized_relative_path("../outside.lisp"));
        assert!(!is_normalized_relative_path("/absolute.lisp"));
        assert!(!is_normalized_relative_path("a/./b.lisp"));
        assert!(!valid_location(DebugLocationKind::StackSlot, 4, 4));
        assert!(!valid_location(DebugLocationKind::SavedRegister, 16, 4));
    }
}
