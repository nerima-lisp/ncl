use crate::Stream;
use crate::value::value_stream::StreamKind;

impl Stream {
    pub(crate) fn write(&mut self, text: &str) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Output {
                buffer,
                position,
                at_line_start,
                output_dirty,
                ..
            } => {
                let mut characters: Vec<char> = buffer.chars().collect();
                for character in text.chars() {
                    if *position < characters.len() {
                        characters[*position] = character;
                    } else {
                        characters.push(character);
                    }
                    *position += 1;
                }
                *buffer = characters.into_iter().collect();
                if let Some(character) = text.chars().last() {
                    *at_line_start = character == '\n';
                }
                *output_dirty = true;
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
                at_line_start,
                output_dirty,
                ..
            } => {
                pushback.take();
                for character in text.chars() {
                    if *position < characters.len() {
                        characters[*position] = character;
                    } else {
                        characters.push(character);
                    }
                    *position += 1;
                }
                if let Some(character) = text.chars().last() {
                    *at_line_start = character == '\n';
                }
                *output_dirty = true;
                true
            }
            StreamKind::Input { .. } => false,
        }
    }

    pub(crate) fn write_byte(&mut self, byte: u8) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::BinaryOutput {
                bytes,
                position,
                output_dirty,
                ..
            } => {
                if *position < bytes.len() {
                    bytes[*position] = byte;
                } else {
                    bytes.push(byte);
                }
                *position += 1;
                *output_dirty = true;
                true
            }
            StreamKind::BinaryIo {
                bytes,
                position,
                pushback,
                output_dirty,
                ..
            } => {
                pushback.take();
                if *position < bytes.len() {
                    bytes[*position] = byte;
                } else {
                    bytes.push(byte);
                }
                *position += 1;
                *output_dirty = true;
                true
            }
            StreamKind::Input { .. }
            | StreamKind::BinaryInput { .. }
            | StreamKind::Probe
            | StreamKind::Io { .. }
            | StreamKind::Output { .. } => false,
        }
    }

    pub(crate) fn fresh_line(&mut self) -> Option<bool> {
        if self.closed {
            return None;
        }
        let at_line_start = match &self.kind {
            StreamKind::Output { at_line_start, .. } | StreamKind::Io { at_line_start, .. } => {
                *at_line_start
            }
            StreamKind::Input { .. }
            | StreamKind::BinaryInput { .. }
            | StreamKind::Probe
            | StreamKind::BinaryOutput { .. }
            | StreamKind::BinaryIo { .. } => return None,
        };
        if at_line_start {
            return Some(false);
        }
        if self.write("\n") { Some(true) } else { None }
    }

    pub(crate) fn take_output(&mut self) -> Option<String> {
        let StreamKind::Output {
            buffer,
            committed_buffer,
            position,
            committed_position,
            at_line_start,
            committed_at_line_start,
            output_dirty,
            file_path: None,
        } = &mut self.kind
        else {
            return None;
        };
        let output = std::mem::take(buffer);
        *committed_buffer = String::new();
        *position = 0;
        *committed_position = 0;
        *at_line_start = true;
        *committed_at_line_start = true;
        *output_dirty = false;
        Some(output)
    }

    pub(crate) fn close(&mut self, abort: bool) -> Result<(), std::io::Error> {
        if self.closed {
            return Ok(());
        }
        if !abort {
            self.flush_output()?;
            if let Some(path) = self.delete_on_close.take() {
                std::fs::remove_file(path)?;
            }
        }
        self.closed = true;
        Ok(())
    }

    pub(crate) fn set_delete_on_close(&mut self, path: std::path::PathBuf) {
        self.delete_on_close = Some(path);
    }
}
