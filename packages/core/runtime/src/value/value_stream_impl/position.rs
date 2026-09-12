use crate::Stream;
use crate::value::value_stream::StreamKind;

fn at_line_start(characters: &[char], position: usize) -> bool {
    position == 0 || characters.get(position - 1) == Some(&'\n')
}

impl Stream {
    pub(crate) fn file_position(&self) -> Option<usize> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                position, pushback, ..
            } => Some(position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::BinaryInput {
                position, pushback, ..
            } => Some(position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::Probe => None,
            StreamKind::Io {
                position, pushback, ..
            } => Some(position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::BinaryIo {
                position, pushback, ..
            } => Some(position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::Output { position, .. } => Some(*position),
            StreamKind::BinaryOutput { position, .. } => Some(*position),
        }
    }

    pub(crate) fn file_end_position(&self) -> Option<usize> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input { characters, .. } => Some(characters.len()),
            StreamKind::BinaryInput { bytes, .. } => Some(bytes.len()),
            StreamKind::Probe => None,
            StreamKind::Io { characters, .. } => Some(characters.len()),
            StreamKind::Output { buffer, .. } => Some(buffer.chars().count()),
            StreamKind::BinaryOutput { bytes, .. } => Some(bytes.len()),
            StreamKind::BinaryIo { bytes, .. } => Some(bytes.len()),
        }
    }

    pub(crate) fn set_file_position(&mut self, position: usize) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Input {
                characters,
                position: current,
                pushback,
                ..
            } => {
                if position > characters.len() {
                    return false;
                }
                *current = position;
                *pushback = None;
                true
            }
            StreamKind::BinaryInput {
                bytes,
                position: current,
                pushback,
                ..
            } => {
                if position > bytes.len() {
                    return false;
                }
                *current = position;
                *pushback = None;
                true
            }
            StreamKind::Probe => false,
            StreamKind::Io {
                characters,
                position: current,
                pushback,
                at_line_start: line_start,
                ..
            } => {
                if position > characters.len() {
                    return false;
                }
                *current = position;
                *pushback = None;
                *line_start = at_line_start(characters, position);
                true
            }
            StreamKind::Output {
                buffer,
                position: current,
                at_line_start: line_start,
                ..
            } => {
                let length = buffer.chars().count();
                if position > length {
                    return false;
                }
                *current = position;
                let characters: Vec<char> = buffer.chars().collect();
                *line_start = at_line_start(&characters, position);
                true
            }
            StreamKind::BinaryOutput {
                bytes,
                position: current,
                ..
            } => {
                if position > bytes.len() {
                    return false;
                }
                *current = position;
                true
            }
            StreamKind::BinaryIo {
                bytes,
                position: current,
                pushback,
                ..
            } => {
                if position > bytes.len() {
                    return false;
                }
                *current = position;
                *pushback = None;
                true
            }
        }
    }
}
