use super::{Stream, StreamKind};

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
                ..
            } => {
                for character in text.chars() {
                    if *position < buffer.len() {
                        buffer[*position] = character;
                    } else {
                        buffer.push(character);
                    }
                    *position += 1;
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
            StreamKind::TwoWay { output, .. } | StreamKind::Echo { output, .. } => {
                output.borrow_mut().write(text)
            }
            StreamKind::Broadcast { streams } => {
                let mut success = true;
                for stream in streams {
                    success &= stream.borrow_mut().write(text);
                }
                success
            }
            StreamKind::Input { .. } | StreamKind::Probe => false,
            StreamKind::Concatenated { .. } => false,
        }
    }

    pub(crate) fn at_line_start(&self) -> Option<bool> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Output { at_line_start, .. } | StreamKind::Io { at_line_start, .. } => {
                Some(*at_line_start)
            }
            StreamKind::TwoWay { output, .. } | StreamKind::Echo { output, .. } => {
                output.borrow().at_line_start()
            }
            StreamKind::Broadcast { streams } => {
                let mut at_line_start = true;
                for stream in streams {
                    at_line_start &= stream.borrow().at_line_start()?;
                }
                Some(at_line_start)
            }
            StreamKind::Input { .. } | StreamKind::Concatenated { .. } | StreamKind::Probe => None,
        }
    }

    pub(crate) fn clear_output(&mut self) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Output {
                buffer,
                position,
                at_line_start,
                ..
            } => {
                buffer.clear();
                *position = 0;
                *at_line_start = true;
                true
            }
            StreamKind::Io { .. } => true,
            StreamKind::TwoWay { output, .. } | StreamKind::Echo { output, .. } => {
                output.borrow_mut().clear_output()
            }
            StreamKind::Broadcast { streams } => {
                let mut success = true;
                for stream in streams {
                    success &= stream.borrow_mut().clear_output();
                }
                success
            }
            StreamKind::Input { .. } | StreamKind::Concatenated { .. } | StreamKind::Probe => false,
        }
    }

    pub(crate) fn flush(&mut self) -> Result<(), std::io::Error> {
        if self.closed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "stream is closed",
            ));
        }
        match &self.kind {
            StreamKind::Output {
                buffer,
                file_path: Some(path),
                ..
            } => {
                let source: String = buffer.iter().collect();
                std::fs::write(path.as_ref(), source.as_bytes())
            }
            StreamKind::Output { .. } => Ok(()),
            StreamKind::Io {
                characters,
                file_path,
                ..
            } => {
                let source: String = characters.iter().collect();
                std::fs::write(file_path.as_ref(), source.as_bytes())
            }
            StreamKind::TwoWay { output, .. } | StreamKind::Echo { output, .. } => {
                output.borrow_mut().flush()
            }
            StreamKind::Broadcast { streams } => {
                for stream in streams {
                    stream.borrow_mut().flush()?;
                }
                Ok(())
            }
            StreamKind::Input { .. } | StreamKind::Concatenated { .. } | StreamKind::Probe => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "stream is not an output stream",
                ))
            }
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
            StreamKind::TwoWay { output, .. } | StreamKind::Echo { output, .. } => {
                return output.borrow_mut().fresh_line();
            }
            StreamKind::Broadcast { streams } => {
                let mut wrote = false;
                for stream in streams {
                    let result = stream.borrow_mut().fresh_line()?;
                    wrote |= result;
                }
                return Some(wrote);
            }
            StreamKind::Input { .. } | StreamKind::Probe => return None,
            StreamKind::Concatenated { .. } => return None,
        };
        if at_line_start {
            return Some(false);
        }
        if self.write("\n") {
            Some(true)
        } else {
            None
        }
    }

    pub(crate) fn take_output(&mut self) -> Option<String> {
        let StreamKind::Output {
            buffer,
            position,
            at_line_start,
            file_path: None,
            ..
        } = &mut self.kind
        else {
            return None;
        };
        let output: String = std::mem::take(buffer).into_iter().collect();
        *position = 0;
        *at_line_start = true;
        Some(output)
    }

    pub(crate) fn close(&mut self, abort: bool) -> Result<(), std::io::Error> {
        if self.closed {
            return Ok(());
        }
        if !abort {
            if let StreamKind::Output {
                buffer,
                file_path: Some(path),
                ..
            } = &self.kind
            {
                let source: String = buffer.iter().collect();
                std::fs::write(path.as_ref(), source.as_bytes())?;
            }
            if let StreamKind::Io {
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
