use super::{Stream, StreamKind};

impl Stream {
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
            StreamKind::TwoWay { input, .. } => input.borrow_mut().read_char(),
            StreamKind::Echo { input, output } => {
                let character = input.borrow_mut().read_char();
                if let Some(character) = character {
                    let text = character.to_string();
                    let _ = output.borrow_mut().write(&text);
                }
                character
            }
            StreamKind::Concatenated { streams, current } => {
                while *current < streams.len() {
                    if let Some(character) = streams[*current].borrow_mut().read_char() {
                        return Some(character);
                    }
                    *current += 1;
                }
                None
            }
            StreamKind::Output { .. } | StreamKind::Probe => None,
            StreamKind::Broadcast { .. } => None,
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
            StreamKind::TwoWay { input, .. } | StreamKind::Echo { input, .. } => {
                input.borrow().peek_char()
            }
            StreamKind::Concatenated { streams, current } => streams
                .iter()
                .skip(*current)
                .find_map(|stream| stream.borrow().peek_char()),
            StreamKind::Output { .. } | StreamKind::Probe => None,
            StreamKind::Broadcast { .. } => None,
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
            StreamKind::TwoWay { input, .. } | StreamKind::Echo { input, .. } => {
                input.borrow_mut().unread_char(character)
            }
            StreamKind::Concatenated { streams, current } => {
                if *current < streams.len() {
                    if streams[*current].borrow_mut().unread_char(character) {
                        return true;
                    }
                }
                if let Some(index) = current.checked_sub(1) {
                    streams[index].borrow_mut().unread_char(character)
                } else {
                    false
                }
            }
            StreamKind::Output { .. } | StreamKind::Probe => false,
            StreamKind::Broadcast { .. } => false,
        }
    }

    pub(crate) fn clear_input(&mut self) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Input { pushback, .. } | StreamKind::Io { pushback, .. } => {
                pushback.take();
                true
            }
            StreamKind::TwoWay { input, .. } | StreamKind::Echo { input, .. } => {
                input.borrow_mut().clear_input()
            }
            StreamKind::Concatenated { streams, current } => streams
                .get(*current)
                .map_or(true, |stream| stream.borrow_mut().clear_input()),
            StreamKind::Output { .. } | StreamKind::Broadcast { .. } | StreamKind::Probe => false,
        }
    }
}
