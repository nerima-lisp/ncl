use std::collections::HashSet;

use crate::{Form, ReadError};

// Keep the recursive first reader below the platform stack limit.
pub const MAX_NESTING_DEPTH: usize = 256;
pub const DEFAULT_FEATURES: &[&str] = &["NCL", "COMMON-LISP"];

pub struct Reader<'source> {
    source: &'source str,
    position: usize,
    nesting_depth: usize,
    features: HashSet<String>,
}

fn normalize_feature_name(feature: &str) -> String {
    feature.trim_start_matches(':').to_ascii_uppercase()
}

impl<'source> Reader<'source> {
    pub fn new(source: &'source str) -> Self {
        Self::with_features(source, &[])
    }

    pub fn with_features(source: &'source str, features: &[&str]) -> Self {
        Self {
            source,
            position: 0,
            nesting_depth: 0,
            features: features
                .iter()
                .map(|feature| normalize_feature_name(feature))
                .collect(),
        }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn read_form(&mut self) -> Result<Option<Form>, ReadError> {
        self.skip_ignored()?;
        if self.position >= self.source.len() {
            return Ok(None);
        }
        self.parse_form()
    }

    pub fn consume_one_whitespace_after_form(&mut self) {
        if let Some(character) = self.peek_char()
            && character.is_whitespace()
        {
            self.position += character.len_utf8();
        }
    }

    pub fn read_all(&mut self) -> Result<Vec<Form>, ReadError> {
        let mut forms = Vec::new();
        while let Some(form) = self.read_form()? {
            forms.push(form);
        }
        Ok(forms)
    }
}

mod atoms;
mod comments;
mod core;
mod dispatch;
mod features;
mod literals;
mod sequences;

pub fn read(source: &str) -> Result<Vec<Form>, ReadError> {
    Reader::with_features(source, DEFAULT_FEATURES).read_all()
}

pub fn read_with_features(source: &str, features: &[&str]) -> Result<Vec<Form>, ReadError> {
    Reader::with_features(source, features).read_all()
}
