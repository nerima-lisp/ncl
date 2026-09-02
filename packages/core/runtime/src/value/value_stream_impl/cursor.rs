use crate::Stream;
use crate::value::value_stream::StreamKind;

impl Stream {
    pub(crate) fn position(&self) -> Option<usize> {
        if self.closed { return None; }
        if let Some(byte_data) = &self.byte_data {
            return Some(match byte_data {
                super::super::value_stream::ByteStreamData::Input { position, .. } => *position,
                super::super::value_stream::ByteStreamData::Io { position, .. } => *position,
                super::super::value_stream::ByteStreamData::Output { position, .. } => *position,
            });
        }
        match &self.kind {
            StreamKind::Input { position, pushback, .. } | StreamKind::Io { position, pushback, .. } => {
                Some(position.saturating_sub(usize::from(pushback.is_some())))
            }
            StreamKind::Output { buffer, .. } => Some(buffer.chars().count()),
        }
    }

    pub(crate) fn length(&self) -> Option<usize> {
        if self.closed { return None; }
        if let Some(byte_data) = &self.byte_data {
            return Some(match byte_data {
                super::super::value_stream::ByteStreamData::Input { bytes, .. } => bytes.len(),
                super::super::value_stream::ByteStreamData::Io { bytes, .. } => bytes.len(),
                super::super::value_stream::ByteStreamData::Output { bytes, .. } => bytes.len(),
            });
        }
        match &self.kind {
            StreamKind::Input { characters, .. } => Some(characters.len()),
            StreamKind::Io { characters, .. } => Some(characters.len()),
            StreamKind::Output { buffer, .. } => Some(buffer.chars().count()),
        }
    }

    pub(crate) fn set_position(&mut self, position: usize) -> bool {
        if self.closed { return false; }
        if let Some(byte_data) = &mut self.byte_data {
            return match byte_data {
                super::super::value_stream::ByteStreamData::Input { bytes, position: cursor } => {
                    if position > bytes.len() { return false; }
                    *cursor = position; true
                }
                super::super::value_stream::ByteStreamData::Io { bytes, position: cursor, .. } => {
                    if position > bytes.len() { return false; }
                    *cursor = position; true
                }
                super::super::value_stream::ByteStreamData::Output { bytes, position: cursor, .. } => {
                    if position > bytes.len() { return false; }
                    *cursor = position; true
                },
            };
        }
        match &mut self.kind {
            StreamKind::Input { characters, position: cursor, pushback, .. } => {
                if position > characters.len() { return false; }
                *cursor = position; pushback.take(); true
            }
            StreamKind::Io { characters, position: cursor, pushback, .. } => {
                if position > characters.len() { return false; }
                *cursor = position; pushback.take(); true
            }
            StreamKind::Output { .. } => false,
        }
    }

    pub(crate) fn read_char(&mut self) -> Option<char> {
        if self.closed {
            return None;
        }
        match &mut self.kind {
            StreamKind::Input {
                characters,
                position,
                pushback,
                ..
            } => {
                if let Some(character) = pushback.take() {
                    return Some(character);
                }
                let character = characters.get(*position).copied()?;
                *position += 1;
                Some(character)
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                if let Some(character) = pushback.take() {
                    return Some(character);
                }
                let character = characters.get(*position).copied()?;
                *position += 1;
                Some(character)
            }
            StreamKind::Output { .. } => None,
        }
    }

    pub(crate) fn peek_char(&self) -> Option<char> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                characters,
                position,
                pushback,
                ..
            } => {
                if let Some(character) = pushback {
                    return Some(*character);
                }
                characters.get(*position).copied()
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                if let Some(character) = pushback {
                    return Some(*character);
                }
                characters.get(*position).copied()
            }
            StreamKind::Output { .. } => None,
        }
    }

    pub(crate) fn unread_char(&mut self, character: char) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Input {
                characters,
                position,
                pushback,
                ..
            } => {
                if pushback.is_some() || *position == 0 {
                    return false;
                }
                if characters.get(*position - 1).copied() != Some(character) {
                    return false;
                }
                *pushback = Some(character);
                true
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                if pushback.is_some() || *position == 0 {
                    return false;
                }
                if characters.get(*position - 1).copied() != Some(character) {
                    return false;
                }
                *pushback = Some(character);
                true
            }
            StreamKind::Output { .. } => false,
        }
    }
}
