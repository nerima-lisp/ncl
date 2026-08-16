use std::path::PathBuf;
use std::rc::Rc;

pub struct Stream {
    kind: StreamKind,
    closed: bool,
}

enum StreamKind {
    Input {
        characters: Rc<Vec<char>>,
        base_position: usize,
        position: usize,
        pushback: Option<char>,
        file: bool,
    },
    Io {
        characters: Vec<char>,
        position: usize,
        pushback: Option<char>,
        at_line_start: bool,
        file_path: Rc<PathBuf>,
    },
    Output {
        buffer: String,
        at_line_start: bool,
        file_path: Option<Rc<PathBuf>>,
    },
}

impl Stream {
    pub(super) fn input(source: &str, start: usize, end: usize) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().skip(start).take(end - start).collect()),
                base_position: start,
                position: 0,
                pushback: None,
                file: false,
            },
            closed: false,
        }
    }

    pub(super) fn file_input(source: String) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().collect()),
                base_position: 0,
                position: 0,
                pushback: None,
                file: true,
            },
            closed: false,
        }
    }

    pub(super) fn file_io(path: PathBuf, source: String, append: bool) -> Self {
        let characters: Vec<char> = source.chars().collect();
        let position = if append { characters.len() } else { 0 };
        let at_line_start = if position == 0 {
            true
        } else {
            characters.get(position - 1) == Some(&'\n')
        };
        Self {
            kind: StreamKind::Io {
                characters,
                position,
                pushback: None,
                at_line_start,
                file_path: Rc::new(path),
            },
            closed: false,
        }
    }

    pub(super) fn output() -> Self {
        Self {
            kind: StreamKind::Output {
                buffer: String::new(),
                at_line_start: true,
                file_path: None,
            },
            closed: false,
        }
    }

    pub(super) fn file_output(path: PathBuf, initial: String) -> Self {
        let at_line_start = initial.ends_with('\n');
        Self {
            kind: StreamKind::Output {
                buffer: initial,
                at_line_start,
                file_path: Some(Rc::new(path)),
            },
            closed: false,
        }
    }

    pub(crate) fn kind_name(&self) -> &'static str {
        match &self.kind {
            StreamKind::Input { file, .. } => {
                if *file {
                    "FILE-INPUT-STREAM"
                } else {
                    "STRING-INPUT-STREAM"
                }
            }
            StreamKind::Io { .. } => "FILE-IO-STREAM",
            StreamKind::Output { file_path, .. } => {
                if file_path.is_some() {
                    "FILE-OUTPUT-STREAM"
                } else {
                    "STRING-OUTPUT-STREAM"
                }
            }
        }
    }

    pub(crate) fn is_input(&self) -> bool {
        matches!(&self.kind, StreamKind::Input { .. } | StreamKind::Io { .. })
    }

    pub(crate) fn is_output(&self) -> bool {
        matches!(
            &self.kind,
            StreamKind::Output { .. } | StreamKind::Io { .. }
        )
    }

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
            StreamKind::Output { .. } => None,
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
            StreamKind::Output { .. } => None,
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
            StreamKind::Output { .. } => false,
        }
    }

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

    pub(crate) fn input_position(&self) -> Option<usize> {
        if self.closed {
            return None;
        }
        match &self.kind {
            StreamKind::Input {
                base_position,
                position,
                pushback,
                ..
            } => Some(base_position + position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::Io {
                position, pushback, ..
            } => Some(position.saturating_sub(usize::from(pushback.is_some()))),
            StreamKind::Output { .. } => None,
        }
    }

    pub(crate) fn write(&mut self, text: &str) -> bool {
        if self.closed {
            return false;
        }
        match &mut self.kind {
            StreamKind::Output {
                buffer,
                at_line_start,
                ..
            } => {
                buffer.push_str(text);
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
            if let StreamKind::Output {
                buffer,
                file_path: Some(path),
                ..
            } = &self.kind
            {
                std::fs::write(path.as_ref(), buffer.as_bytes())?;
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
