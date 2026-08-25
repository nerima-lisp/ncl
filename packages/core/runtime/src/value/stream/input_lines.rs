use super::{Stream, StreamKind};

impl Stream {
    pub(crate) fn read_line(&mut self) -> Option<(String, bool)> {
        let first = self.read_char()?;
        let mut line = String::new();
        let mut character = first;
        loop {
            if character == '\n' {
                return Some((line, false));
            }
            line.push(character);
            match self.read_char() {
                Some(next) => character = next,
                None => return Some((line, true)),
            }
        }
    }

    pub(crate) fn remaining_input(&self) -> Option<String> {
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
                let mut source = String::new();
                if let Some(character) = pushback {
                    source.push(*character);
                }
                source.extend(characters.iter().skip(*position).copied());
                Some(source)
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                let mut source = String::new();
                if let Some(character) = pushback {
                    source.push(*character);
                }
                source.extend(characters.iter().skip(*position).copied());
                Some(source)
            }
            StreamKind::TwoWay { input, .. } | StreamKind::Echo { input, .. } => {
                input.borrow().remaining_input()
            }
            StreamKind::Concatenated { streams, current } => {
                let mut source = String::new();
                for stream in streams.iter().skip(*current) {
                    source.push_str(&stream.borrow().remaining_input()?);
                }
                Some(source)
            }
            StreamKind::Output { .. } | StreamKind::Probe => None,
            StreamKind::Broadcast { .. } => None,
        }
    }
}
