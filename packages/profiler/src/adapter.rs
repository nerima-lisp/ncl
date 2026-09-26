//! Runtime adapters for sampling and rendering a domain profile.

use std::collections::BTreeMap;

use ncl_sys::{CodeRegistry, Thread, walk_frame_headers};

use crate::domain::{FrameId, Profile, Sample, SampleError};

/// Names assigned to frames while adapting runtime code metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameCatalog {
    names: BTreeMap<FrameId, String>,
    ids: BTreeMap<String, FrameId>,
}

impl FrameCatalog {
    /// Create an empty frame catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a frame name and return its stable session-local identifier.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty name or an exhausted identifier space.
    pub fn intern(&mut self, name: &str) -> Result<FrameId, SampleError> {
        if name.is_empty() {
            return Err(SampleError::EmptyFrameName);
        }
        if let Some(id) = self.ids.get(name).copied() {
            return Ok(id);
        }
        let id =
            FrameId::new(u32::try_from(self.names.len()).map_err(|_| SampleError::TooManyFrames)?);
        let owned = name.to_owned();
        self.ids.insert(owned.clone(), id);
        self.names.insert(id, owned);
        Ok(id)
    }

    /// Look up a frame name by identifier.
    #[must_use]
    pub fn name(&self, id: FrameId) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }
}

/// Bounded adapter from a published thread frame snapshot to domain samples.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreadSampler {
    max_words: usize,
    max_frames: usize,
}

impl ThreadSampler {
    /// Configure bounded frame-snapshot and chain capacities.
    ///
    /// # Errors
    ///
    /// Returns [`SampleError::InvalidSamplerCapacity`] when either bound is zero.
    pub const fn new(max_words: usize, max_frames: usize) -> Result<Self, SampleError> {
        if max_words == 0 || max_frames == 0 {
            return Err(SampleError::InvalidSamplerCapacity);
        }
        Ok(Self {
            max_words,
            max_frames,
        })
    }

    /// Sample a thread's published frame snapshot.
    ///
    /// The sampler resolves return PCs through the existing code registry and
    /// uses its function names as catalog keys. It never exposes Lisp words to
    /// the domain layer.
    ///
    /// # Errors
    ///
    /// Returns a sampling error when a frame lacks a resolvable code object,
    /// safepoint map, or function name.
    pub fn sample(
        &self,
        thread: &Thread,
        registry: &CodeRegistry,
        catalog: &mut FrameCatalog,
    ) -> Result<Sample, SampleError> {
        let mut words = Vec::new();
        for index in 0..self.max_words {
            let Some(word) = thread.frame_word(index) else {
                break;
            };
            words.push(word);
        }
        let headers = walk_frame_headers(&words, 0, self.max_frames);
        let mut frames = Vec::with_capacity(headers.len());
        for header in headers {
            let (metadata, offset) = registry
                .find(header.return_pc)
                .ok_or(SampleError::UnknownCodeAddress)?;
            if metadata.safepoint_map.find_map(offset).is_none() {
                return Err(SampleError::UnknownSafepoint);
            }
            frames.push(catalog.intern(&metadata.function_name)?);
        }
        Sample::new(frames)
    }
}

/// Supported report encodings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportFormat {
    /// One line per leaf frame.
    Flat,
    /// One line per inclusive frame count.
    Cumulative,
    /// One line per caller/callee edge.
    Callgraph,
    /// One line per folded root-to-leaf stack.
    Folded,
}

/// UTF-8 report output with an explicit type at the report boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportText(String);

impl ReportText {
    /// Render a profile with names from a frame catalog.
    #[must_use]
    pub fn render(profile: &Profile, catalog: &FrameCatalog, format: ReportFormat) -> Self {
        let mut output = String::new();
        match format {
            ReportFormat::Flat => {
                for entry in profile.flat() {
                    append_line(&mut output, &[name(catalog, entry.frame)], entry.samples);
                }
            }
            ReportFormat::Cumulative => {
                for entry in profile.cumulative() {
                    append_line(&mut output, &[name(catalog, entry.frame)], entry.samples);
                }
            }
            ReportFormat::Callgraph => {
                for edge in profile.callgraph() {
                    append_line(
                        &mut output,
                        &[name(catalog, edge.caller), name(catalog, edge.callee)],
                        edge.samples,
                    );
                }
            }
            ReportFormat::Folded => {
                for entry in profile.folded() {
                    let names = entry
                        .frames
                        .iter()
                        .map(|frame| name(catalog, *frame))
                        .collect::<Vec<_>>();
                    append_line(&mut output, &names, entry.samples);
                }
            }
        }
        Self(output)
    }

    /// Borrow the rendered report as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn name(catalog: &FrameCatalog, frame: FrameId) -> &str {
    catalog.name(frame).unwrap_or("<unknown>")
}

fn append_line(output: &mut String, names: &[&str], samples: u64) {
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(&names.join(";"));
    output.push(' ');
    output.push_str(&samples.to_string());
}

#[cfg(test)]
mod tests {
    use super::{FrameCatalog, ReportFormat, ReportText};
    use crate::domain::{FrameId, Profile, Sample};

    #[test]
    fn renders_folded_output_with_catalog_names() {
        let mut catalog = FrameCatalog::new();
        let root = catalog.intern("root");
        let leaf = catalog.intern("leaf");
        assert!(root.is_ok());
        assert!(leaf.is_ok());
        let mut profile = Profile::new();
        let sample = match (root, leaf) {
            (Ok(root), Ok(leaf)) => Sample::new(vec![root, leaf]),
            _ => return,
        };
        assert!(sample.is_ok());
        if let Ok(sample) = sample {
            profile.push(sample);
        }
        let report = ReportText::render(&profile, &catalog, ReportFormat::Folded);
        assert_eq!(report.as_str(), "root;leaf 1");
    }

    #[test]
    fn two_sampling_workers_settle_before_the_watchdog_deadline() {
        use std::sync::mpsc::channel;
        use std::time::Duration;

        let (sender, receiver) = channel();
        let mut handles = Vec::new();
        for leaf in [2_u32, 3_u32] {
            let sender = sender.clone();
            handles.push(std::thread::spawn(move || {
                let sample = Sample::new(vec![FrameId::new(1), FrameId::new(leaf)]);
                let _ = sender.send(sample);
            }));
        }
        drop(sender);

        let first = receiver.recv_timeout(Duration::from_secs(1));
        let second = receiver.recv_timeout(Duration::from_secs(1));
        assert!(first.is_ok());
        assert!(second.is_ok());
        for handle in handles {
            assert!(handle.join().is_ok());
        }
    }
}
