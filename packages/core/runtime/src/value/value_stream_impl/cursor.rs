use crate::Stream;
use crate::value::value_stream::StreamKind;

impl Stream {
    pub(crate) fn position(&self) -> Option<usize> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                position, pushback, ..
            }
            | StreamKind::Io {
                position, pushback, ..
            } => Some(position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::Output { position, .. }
            | StreamKind::BinaryOutput { position, .. }
            | StreamKind::BinaryIo { position, .. } => Some(*position),
            StreamKind::BinaryInput {
                position, pushback, ..
            } => Some(position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::Probe => None,
        }
    }

    pub(crate) fn length(&self) -> Option<usize> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input { characters, .. } => Some(characters.len()),
            StreamKind::BinaryInput { bytes, .. } => Some(bytes.len()),
            StreamKind::BinaryOutput { bytes, .. } | StreamKind::BinaryIo { bytes, .. } => {
                Some(bytes.len())
            }
            StreamKind::Io { characters, .. } => Some(characters.len()),
            StreamKind::Output { buffer, .. } => Some(buffer.chars().count()),
            StreamKind::Probe => None,
        }
    }

    pub(crate) fn set_position(&mut self, position: usize) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Input {
                characters,
                position: cursor,
                pushback,
                ..
            } => {
                if position > characters.len() {
                    return false;
                }
                *cursor = position;
                pushback.take();
                true
            }
            StreamKind::Io {
                characters,
                position: cursor,
                pushback,
                ..
            } => {
                if position > characters.len() {
                    return false;
                }
                *cursor = position;
                pushback.take();
                true
            }
            StreamKind::BinaryInput {
                bytes,
                position: cursor,
                pushback,
            } => {
                if position > bytes.len() {
                    return false;
                }
                *cursor = position;
                pushback.take();
                true
            }
            StreamKind::BinaryOutput {
                bytes,
                position: cursor,
                ..
            } => {
                if position > bytes.len() {
                    return false;
                }
                *cursor = position;
                true
            }
            StreamKind::BinaryIo {
                bytes,
                position: cursor,
                pushback,
                ..
            } => {
                if position > bytes.len() {
                    return false;
                }
                *cursor = position;
                pushback.take();
                true
            }
            StreamKind::Output { .. } | StreamKind::Probe => false,
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
            StreamKind::Probe
            | StreamKind::BinaryInput { .. }
            | StreamKind::BinaryOutput { .. }
            | StreamKind::BinaryIo { .. } => None,
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
            StreamKind::Probe
            | StreamKind::BinaryInput { .. }
            | StreamKind::BinaryOutput { .. }
            | StreamKind::BinaryIo { .. } => None,
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
            StreamKind::Probe
            | StreamKind::BinaryInput { .. }
            | StreamKind::BinaryOutput { .. }
            | StreamKind::BinaryIo { .. } => false,
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

    pub(crate) fn read_byte(&mut self) -> Option<u8> {
        if self.closed {
            return None;
        }
        match &mut self.kind {
            StreamKind::BinaryInput {
                bytes,
                position,
                pushback,
            } => {
                if let Some(byte) = pushback.take() {
                    return Some(byte);
                }
                let byte = bytes.get(*position).copied()?;
                *position += 1;
                Some(byte)
            }
            StreamKind::BinaryIo {
                bytes,
                position,
                pushback,
                ..
            } => {
                if let Some(byte) = pushback.take() {
                    return Some(byte);
                }
                let byte = bytes.get(*position).copied()?;
                *position += 1;
                Some(byte)
            }
            StreamKind::Input { .. }
            | StreamKind::Probe
            | StreamKind::Io { .. }
            | StreamKind::Output { .. }
            | StreamKind::BinaryOutput { .. } => None,
        }
    }
}
