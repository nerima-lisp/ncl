use crate::Stream;
use crate::value::value_stream::StreamKind;

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
            StreamKind::Output { .. } => None,
        }
    }

    pub(crate) fn consume_input(&mut self, count: usize) -> bool {
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
                let available =
                    usize::from(pushback.is_some()) + characters.len().saturating_sub(*position);
                if count > available {
                    return false;
                }
                if count == 0 {
                    return true;
                }
                let mut remaining = count;
                if pushback.take().is_some() {
                    remaining -= 1;
                }
                *position += remaining;
                true
            }
            StreamKind::Io {
                characters,
                position,
                pushback,
                ..
            } => {
                let available =
                    usize::from(pushback.is_some()) + characters.len().saturating_sub(*position);
                if count > available {
                    return false;
                }
                if count == 0 {
                    return true;
                }
                let mut remaining = count;
                if pushback.take().is_some() {
                    remaining -= 1;
                }
                *position += remaining;
                true
            }
            StreamKind::Output { .. } => false,
        }
    }
}
