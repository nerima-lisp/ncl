use std::path::PathBuf;
use std::rc::Rc;

use super::Stream;
use super::value_stream::StreamKind;

impl Stream {
    pub(super) fn input(source: &str, start: usize, end: usize) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().skip(start).take(end - start).collect()),
                position: 0,
                pushback: None,
                file: false,
            },
            closed: false,
        }
    }

    pub(super) fn file_input(source: &str) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().collect()),
                position: 0,
                pushback: None,
                file: true,
            },
            closed: false,
        }
    }

    pub(super) fn file_io(path: PathBuf, source: &str, append: bool) -> Self {
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

    pub(super) const fn output() -> Self {
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

    pub(crate) const fn kind_name(&self) -> &'static str {
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

    pub(crate) const fn is_input(&self) -> bool {
        matches!(&self.kind, StreamKind::Input { .. } | StreamKind::Io { .. })
    }

    pub(crate) const fn is_output(&self) -> bool {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Stream;

    #[test]
    fn input_streams_cover_cursor_and_line_boundaries_from_table_cases() {
        let cases = [
            ("string", Stream::input("ab\ncd", 0, 5)),
            ("file", Stream::file_input("ab\ncd")),
        ];
        for (name, mut stream) in cases {
            assert_eq!(
                stream.kind_name(),
                if name == "file" {
                    "FILE-INPUT-STREAM"
                } else {
                    "STRING-INPUT-STREAM"
                }
            );
            assert!(stream.is_input());
            assert!(!stream.is_output());
            assert_eq!(stream.peek_char(), Some('a'));
            assert_eq!(stream.read_char(), Some('a'));
            assert!(!stream.unread_char('x'));
            assert!(stream.unread_char('a'));
            assert!(!stream.unread_char('a'));
            assert_eq!(stream.read_line(), Some(("ab".into(), false)));
            assert_eq!(stream.remaining_input(), Some("cd".into()));
            assert!(stream.consume_input(1));
            assert_eq!(stream.remaining_input(), Some("d".into()));
            assert!(!stream.consume_input(2));
            assert!(stream.consume_input(0));
            assert_eq!(stream.read_line(), Some(("d".into(), true)));
            assert_eq!(stream.read_line(), None);
        }
    }

    #[test]
    fn output_streams_cover_buffer_and_newline_boundaries_from_table_cases() {
        let cases = [
            ("", true, ""),
            ("prefix", false, "prefix\n"),
            ("prefix\n", true, "prefix\n"),
        ];
        for (initial, at_line_start, expected) in cases {
            let mut stream = Stream::output();
            assert_eq!(stream.kind_name(), "STRING-OUTPUT-STREAM");
            assert!(!stream.is_input());
            assert!(stream.is_output());
            assert!(stream.write(initial));
            assert_eq!(stream.fresh_line(), Some(!at_line_start));
            assert_eq!(stream.take_output(), Some(expected.to_owned()));
            assert_eq!(stream.take_output(), Some(String::new()));
            assert!(stream.write("open"));
            if let Err(error) = stream.close(false) {
                panic!("closing string output stream: {error}");
            }
            assert!(!stream.write("closed"));
            assert_eq!(stream.fresh_line(), None);
        }
    }

    #[test]
    fn file_streams_cover_commit_and_abort_from_table_cases() {
        let base = std::env::temp_dir().join(format!("ncl-stream-{}", std::process::id()));
        let cases = [("output", "written", false), ("abort", "discarded", true)];
        for (suffix, contents, abort) in cases {
            let path = PathBuf::from(format!("{}-{suffix}", base.display()));
            let mut stream = Stream::file_output(path.clone(), String::new());
            assert!(stream.write(contents));
            if let Err(error) = stream.close(abort) {
                panic!("closing file output stream: {error}");
            }
            if abort {
                assert!(!path.exists());
            } else {
                let actual = match std::fs::read_to_string(&path) {
                    Ok(actual) => actual,
                    Err(error) => panic!("reading committed stream output: {error}"),
                };
                assert_eq!(actual, contents);
                if let Err(error) = std::fs::remove_file(&path) {
                    panic!("removing committed stream output: {error}");
                }
            }
        }
    }

    #[test]
    fn io_streams_cover_read_write_append_and_close_from_table_cases() {
        let base = std::env::temp_dir().join(format!("ncl-io-stream-{}", std::process::id()));
        let cases = [(false, "ab", "X", "aX"), (true, "ab", "X", "abX")];
        for (append, source, text, expected) in cases {
            let path = PathBuf::from(format!("{}-{append}", base.display()));
            let mut stream = Stream::file_io(path.clone(), source, append);
            assert_eq!(stream.kind_name(), "FILE-IO-STREAM");
            assert!(stream.is_input());
            assert!(stream.is_output());
            if append {
                assert_eq!(stream.read_char(), None);
            } else {
                assert_eq!(stream.read_char(), Some('a'));
            }
            assert!(stream.write(text));
            if let Err(error) = stream.close(false) {
                panic!("closing file IO stream: {error}");
            }
            let actual = match std::fs::read_to_string(&path) {
                Ok(actual) => actual,
                Err(error) => panic!("reading file IO stream output: {error}"),
            };
            assert_eq!(actual, expected);
            if let Err(error) = std::fs::remove_file(path) {
                panic!("removing file IO stream output: {error}");
            }
        }
    }
}
