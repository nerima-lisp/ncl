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
                destination,
                at_line_start,
                ..
            } => {
                for character in text.chars() {
                    let byte_position = buffer
                        .char_indices()
                        .nth(*position)
                        .map_or(buffer.len(), |(byte_position, _)| byte_position);
                    if *position < buffer.chars().count() {
                        let next_byte_position = buffer
                            .char_indices()
                            .nth(*position + 1)
                            .map_or(buffer.len(), |(next_byte_position, _)| next_byte_position);
                        buffer.replace_range(
                            byte_position..next_byte_position,
                            &character.to_string(),
                        );
                    } else {
                        buffer.push(character);
                    }
                    *position += 1;
                }
                if let Some(destination) = destination {
                    destination.borrow_mut().push_str(text);
                }
                if let Some(character) = text.chars().last() {
                    *at_line_start = character == '\n';
                }
                true
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                at_line_start,
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
                true
            }
            StreamKind::Input { .. } => false,
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
            StreamKind::Input { .. } => return None,
        };
        if at_line_start {
            return Some(false);
        }
        if self.write("\n") { Some(true) } else { None }
    }

    pub(crate) fn clear_output(&mut self) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Output {
                buffer,
                position,
                destination,
                ..
            } => {
                buffer.clear();
                *position = 0;
                if let Some(destination) = destination {
                    destination.borrow_mut().clear();
                }
            }
            StreamKind::Io { .. } => {}
            StreamKind::Input { .. } => return false,
        }
        true
    }

    pub(crate) fn take_output(&mut self) -> Option<String> {
        let StreamKind::Output {
            buffer,
            file_path: None,
            ..
        } = &mut self.kind
        else {
            return None;
        };
        Some(std::mem::take(buffer))
    }

    pub(crate) fn close(&mut self, abort: bool) -> Result<(), std::io::Error> {
        if self.closed {
            return Ok(());
        }
        if !abort {
            if let Some(super::super::value_stream::ByteStreamData::Io {
                bytes, file_path, ..
            }) = &self.byte_data
            {
                std::fs::write(file_path.as_ref(), bytes)?;
            }
            if let Some(super::super::value_stream::ByteStreamData::Output {
                bytes,
                file_path,
                ..
            }) = &self.byte_data
            {
                std::fs::write(file_path.as_ref(), bytes)?;
            }
            if self.byte_data.is_none()
                && let StreamKind::Output {
                    buffer,
                    file_path: Some(path),
                    ..
                } = &self.kind
            {
                std::fs::write(path.as_ref(), buffer.as_bytes())?;
            }
            if self.byte_data.is_none()
                && let StreamKind::Io {
                    characters,
                    file_path,
                    ..
                } = &self.kind
            {
                let source: String = characters.iter().collect();
                std::fs::write(file_path.as_ref(), source.as_bytes())?;
            }
        }
        self.closed = true;
        Ok(())
    }
}
