use super::{Stream, StreamKind};

impl Stream {
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
            StreamKind::TwoWay { input, .. } | StreamKind::Echo { input, .. } => {
                input.borrow_mut().consume_input(count)
            }
            StreamKind::Concatenated { .. } => {
                let Some(remaining) = self.remaining_input() else {
                    return false;
                };
                if count > remaining.chars().count() {
                    return false;
                }
                for _ in 0..count {
                    let _ = self.read_char();
                }
                true
            }
            StreamKind::Output { .. } | StreamKind::Probe => false,
            StreamKind::Broadcast { .. } => false,
        }
    }
}
