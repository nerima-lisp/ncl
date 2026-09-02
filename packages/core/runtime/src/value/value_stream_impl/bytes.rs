use std::path::PathBuf;
use std::rc::Rc;

use super::super::value_stream::{ByteStreamData, Stream, StreamElementType};

impl Stream {
    pub(in crate::value) fn file_byte_input(bytes: Vec<u8>) -> Self {
        Self { kind: super::super::value_stream::StreamKind::Input { characters: Rc::new(Vec::new()), position: 0, pushback: None, file: true }, closed: false, element_type: StreamElementType::UnsignedByte8, byte_data: Some(ByteStreamData::Input { bytes: Rc::new(bytes), position: 0 }) }
    }

    pub(in crate::value) fn file_byte_output(path: PathBuf, bytes: Vec<u8>) -> Self {
        let position = bytes.len();
        Self { kind: super::super::value_stream::StreamKind::Output { buffer: String::new(), destination: None, at_line_start: true, file_path: Some(Rc::new(path.clone())) }, closed: false, element_type: StreamElementType::UnsignedByte8, byte_data: Some(ByteStreamData::Output { bytes, position, file_path: Rc::new(path) }) }
    }

    pub(in crate::value) fn file_byte_io(path: PathBuf, bytes: Vec<u8>, append: bool) -> Self {
        let position = if append { bytes.len() } else { 0 };
        Self { kind: super::super::value_stream::StreamKind::Io { characters: Vec::new(), position: 0, pushback: None, at_line_start: true, file_path: Rc::new(path.clone()) }, closed: false, element_type: StreamElementType::UnsignedByte8, byte_data: Some(ByteStreamData::Io { bytes, position, file_path: Rc::new(path) }) }
    }

    pub(crate) fn read_byte(&mut self) -> Option<u8> {
        match self.byte_data.as_mut()? {
            ByteStreamData::Input { bytes, position } => {
                let byte = bytes.get(*position).copied()?;
                *position += 1;
                Some(byte)
            }
            ByteStreamData::Io { bytes, position, .. } => {
                let byte = bytes.get(*position).copied()?;
                *position += 1;
                Some(byte)
            }
            ByteStreamData::Output { .. } => None,
        }
    }

    pub(crate) fn write_byte(&mut self, byte: u8) -> bool {
        let Some(data) = self.byte_data.as_mut() else { return false };
        match data {
            ByteStreamData::Output { bytes, position, .. } => {
                if *position < bytes.len() {
                    bytes[*position] = byte;
                } else {
                    bytes.push(byte);
                }
                *position += 1;
            }
            ByteStreamData::Io { bytes, position, .. } => {
                if *position < bytes.len() {
                    bytes[*position] = byte;
                } else {
                    bytes.push(byte);
                }
                *position += 1;
            }
            ByteStreamData::Input { .. } => return false,
        }
        true
    }
}
