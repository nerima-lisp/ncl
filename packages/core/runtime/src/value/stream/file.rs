use super::{Stream, StreamKind};

impl Stream {
    pub(crate) fn string_input_position(&self) -> Option<usize> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                file: false,
                start,
                position,
                pushback,
                ..
            } => Some(*start + position.saturating_sub(usize::from(pushback.is_some()))),
            _ => None,
        }
    }

    pub(crate) fn file_position(&self) -> Option<usize> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                file: true,
                position,
                pushback,
                ..
            }
            | StreamKind::Io {
                position, pushback, ..
            } => Some(position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::Output {
                file_path: Some(_),
                position,
                ..
            } => Some(*position),
            _ => None,
        }
    }

    pub(crate) fn file_length(&self) -> Option<usize> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                file: true,
                characters,
                ..
            } => Some(characters.len()),
            StreamKind::Io { characters, .. } => Some(characters.len()),
            StreamKind::Output {
                file_path: Some(_),
                buffer,
                ..
            } => Some(buffer.len()),
            _ => None,
        }
    }

    pub(crate) fn set_file_position(&mut self, position: usize) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Input {
                file: true,
                characters,
                position: current,
                pushback,
                ..
            } => {
                if position > characters.len() {
                    return false;
                }
                *current = position;
                pushback.take();
                true
            }
            StreamKind::Io {
                characters,
                position: current,
                pushback,
                at_line_start,
                ..
            } => {
                if position > characters.len() {
                    return false;
                }
                *current = position;
                pushback.take();
                *at_line_start = position == 0 || characters.get(position - 1) == Some(&'\n');
                true
            }
            StreamKind::Output {
                file_path: Some(_),
                buffer,
                position: current,
                at_line_start,
                ..
            } => {
                if position > buffer.len() {
                    return false;
                }
                *current = position;
                *at_line_start = position == 0 || buffer[position - 1] == '\n';
                true
            }
            _ => false,
        }
    }
}
