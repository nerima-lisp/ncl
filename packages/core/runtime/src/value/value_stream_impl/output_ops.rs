use crate::Stream;
use crate::value::value_stream::StreamKind;

impl Stream {
    pub(crate) fn flush_output(&mut self) -> Result<(), std::io::Error> {
        if self.closed {
            return Ok(());
        }
        match &mut self.kind {
            StreamKind::Io {
                characters,
                committed_characters,
                position,
                committed_position,
                pushback,
                committed_pushback,
                at_line_start,
                committed_at_line_start,
                output_dirty,
                file_path,
            } => {
                let source: String = characters.iter().collect();
                std::fs::write(file_path.as_ref(), source.as_bytes())?;
                *committed_characters = characters.clone();
                *committed_position = *position;
                *committed_pushback = *pushback;
                *committed_at_line_start = *at_line_start;
                *output_dirty = false;
            }
            StreamKind::Output {
                buffer,
                committed_buffer,
                position,
                committed_position,
                at_line_start,
                committed_at_line_start,
                output_dirty,
                file_path,
            } => {
                if let Some(path) = file_path {
                    std::fs::write(path.as_ref(), buffer.as_bytes())?;
                }
                *committed_buffer = buffer.clone();
                *committed_position = *position;
                *committed_at_line_start = *at_line_start;
                *output_dirty = false;
            }
            StreamKind::BinaryOutput {
                bytes,
                committed_bytes,
                position,
                committed_position,
                output_dirty,
                file_path,
            } => {
                std::fs::write(file_path.as_ref(), &*bytes)?;
                *committed_bytes = bytes.clone();
                *committed_position = *position;
                *output_dirty = false;
            }
            StreamKind::BinaryIo {
                bytes,
                committed_bytes,
                position,
                committed_position,
                pushback,
                committed_pushback,
                output_dirty,
                file_path,
            } => {
                std::fs::write(file_path.as_ref(), &*bytes)?;
                *committed_bytes = bytes.clone();
                *committed_position = *position;
                *committed_pushback = *pushback;
                *output_dirty = false;
            }
            StreamKind::Input { .. } | StreamKind::BinaryInput { .. } | StreamKind::Probe => {}
        }
        Ok(())
    }

    pub(crate) fn clear_output(&mut self) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Io {
                characters,
                committed_characters,
                position,
                committed_position,
                pushback,
                committed_pushback,
                at_line_start,
                committed_at_line_start,
                output_dirty,
                ..
            } => {
                if *output_dirty {
                    *characters = committed_characters.clone();
                    *position = *committed_position;
                    *pushback = *committed_pushback;
                    *at_line_start = *committed_at_line_start;
                    *output_dirty = false;
                }
                true
            }
            StreamKind::Output {
                buffer,
                committed_buffer,
                position,
                committed_position,
                at_line_start,
                committed_at_line_start,
                output_dirty,
                ..
            } => {
                if *output_dirty {
                    *buffer = committed_buffer.clone();
                    *position = *committed_position;
                    *at_line_start = *committed_at_line_start;
                    *output_dirty = false;
                }
                true
            }
            StreamKind::BinaryOutput {
                bytes,
                committed_bytes,
                position,
                committed_position,
                output_dirty,
                ..
            } => {
                if *output_dirty {
                    *bytes = committed_bytes.clone();
                    *position = *committed_position;
                    *output_dirty = false;
                }
                true
            }
            StreamKind::BinaryIo {
                bytes,
                committed_bytes,
                position,
                committed_position,
                pushback,
                committed_pushback,
                output_dirty,
                ..
            } => {
                if *output_dirty {
                    *bytes = committed_bytes.clone();
                    *position = *committed_position;
                    *pushback = *committed_pushback;
                    *output_dirty = false;
                }
                true
            }
            StreamKind::Input { .. } | StreamKind::BinaryInput { .. } | StreamKind::Probe => false,
        }
    }
}
