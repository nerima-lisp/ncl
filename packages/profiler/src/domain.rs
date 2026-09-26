//! Pure profile aggregation over logical stack samples.

use std::collections::BTreeMap;
use std::fmt;

/// Stable identifier for a frame in one profile session.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct FrameId(u32);

impl FrameId {
    /// Construct a frame identifier.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Return the numeric identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A logical stack, ordered from root to leaf.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sample {
    frames: Vec<FrameId>,
}

impl Sample {
    /// Build a sample. Empty stacks are rejected because they cannot contribute
    /// to either self or cumulative counts.
    ///
    /// # Errors
    ///
    /// Returns [`SampleError::EmptyStack`] for an empty frame sequence.
    pub fn new(frames: Vec<FrameId>) -> Result<Self, SampleError> {
        if frames.is_empty() {
            return Err(SampleError::EmptyStack);
        }
        Ok(Self { frames })
    }

    /// Return the root-to-leaf frame sequence.
    #[must_use]
    pub fn frames(&self) -> &[FrameId] {
        &self.frames
    }
}

/// Invalid sample input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleError {
    /// A sample must contain at least one frame.
    EmptyStack,
    /// A configured sampler has no usable capacity.
    InvalidSamplerCapacity,
    /// A frame snapshot does not contain a return PC.
    MissingReturnPc,
    /// A return PC is not covered by the code registry.
    UnknownCodeAddress,
    /// A registered code address has no matching safepoint map.
    UnknownSafepoint,
    /// A code object has no usable function name.
    EmptyFrameName,
    /// The session-local frame identifier space is exhausted.
    TooManyFrames,
}

impl fmt::Display for SampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyStack => "sample stack is empty",
            Self::InvalidSamplerCapacity => "sampler capacity must be non-zero",
            Self::MissingReturnPc => "frame snapshot has no return PC",
            Self::UnknownCodeAddress => "return PC is not in the code registry",
            Self::UnknownSafepoint => "return PC has no safepoint map",
            Self::EmptyFrameName => "code object has an empty function name",
            Self::TooManyFrames => "frame identifier space is exhausted",
        };
        f.write_str(message)
    }
}

impl std::error::Error for SampleError {}

/// One frame's self-sample count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlatCount {
    /// Frame receiving the sample at the leaf.
    pub frame: FrameId,
    /// Number of samples whose leaf is this frame.
    pub samples: u64,
}

/// One frame's cumulative sample count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CumulativeCount {
    /// Frame present anywhere in the sample stack.
    pub frame: FrameId,
    /// Number of samples containing this frame.
    pub samples: u64,
}

/// A caller-to-callee edge count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallEdge {
    /// Caller frame.
    pub caller: FrameId,
    /// Direct callee frame.
    pub callee: FrameId,
    /// Number of samples containing this adjacent edge.
    pub samples: u64,
}

/// A folded stack and its count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoldedStack {
    /// Root-to-leaf stack.
    pub frames: Vec<FrameId>,
    /// Number of identical stacks observed.
    pub samples: u64,
}

/// Pure aggregate state for a sequence of samples.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Profile {
    samples: Vec<Sample>,
}

impl Profile {
    /// Create an empty profile.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    /// Add one sample without consulting runtime state.
    pub fn push(&mut self, sample: Sample) {
        self.samples.push(sample);
    }

    /// Number of accepted samples.
    #[must_use]
    pub fn sample_count(&self) -> u64 {
        u64::try_from(self.samples.len()).unwrap_or(u64::MAX)
    }

    /// Aggregate leaf-only counts in frame-id order.
    #[must_use]
    pub fn flat(&self) -> Vec<FlatCount> {
        let mut counts = BTreeMap::new();
        for sample in &self.samples {
            if let Some(frame) = sample.frames().last().copied() {
                *counts.entry(frame).or_insert(0) += 1;
            }
        }
        counts
            .into_iter()
            .map(|(frame, samples)| FlatCount { frame, samples })
            .collect()
    }

    /// Aggregate inclusive counts in frame-id order.
    #[must_use]
    pub fn cumulative(&self) -> Vec<CumulativeCount> {
        let mut counts = BTreeMap::new();
        for sample in &self.samples {
            for frame in sample.frames() {
                *counts.entry(*frame).or_insert(0) += 1;
            }
        }
        counts
            .into_iter()
            .map(|(frame, samples)| CumulativeCount { frame, samples })
            .collect()
    }

    /// Aggregate adjacent caller-to-callee edges.
    #[must_use]
    pub fn callgraph(&self) -> Vec<CallEdge> {
        let mut counts = BTreeMap::new();
        for sample in &self.samples {
            for pair in sample.frames().windows(2) {
                if let [caller, callee] = pair {
                    *counts.entry((*caller, *callee)).or_insert(0) += 1;
                }
            }
        }
        counts
            .into_iter()
            .map(|((caller, callee), samples)| CallEdge {
                caller,
                callee,
                samples,
            })
            .collect()
    }

    /// Aggregate identical root-to-leaf stacks.
    #[must_use]
    pub fn folded(&self) -> Vec<FoldedStack> {
        let mut counts: BTreeMap<Vec<FrameId>, u64> = BTreeMap::new();
        for sample in &self.samples {
            *counts.entry(sample.frames.clone()).or_insert(0) += 1;
        }
        counts
            .into_iter()
            .map(|(frames, samples)| FoldedStack { frames, samples })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameId, Profile, Sample, SampleError};

    #[test]
    fn aggregates_flat_cumulative_callgraph_and_folded_views() {
        let mut profile = Profile::new();
        let samples = [vec![1, 2, 3], vec![1, 2, 4], vec![1, 2, 3]]
            .into_iter()
            .map(|frames| Sample::new(frames.into_iter().map(FrameId::new).collect()))
            .collect::<Result<Vec<_>, _>>();
        assert!(samples.is_ok());
        if let Ok(samples) = samples {
            for sample in samples {
                profile.push(sample);
            }
        }

        assert_eq!(profile.sample_count(), 3);
        assert_eq!(
            profile
                .flat()
                .iter()
                .map(|entry| (entry.frame.get(), entry.samples))
                .collect::<Vec<_>>(),
            vec![(3, 2), (4, 1)]
        );
        assert_eq!(
            profile
                .cumulative()
                .iter()
                .map(|entry| (entry.frame.get(), entry.samples))
                .collect::<Vec<_>>(),
            vec![(1, 3), (2, 3), (3, 2), (4, 1)]
        );
        assert_eq!(
            profile
                .callgraph()
                .iter()
                .map(|edge| (edge.caller.get(), edge.callee.get(), edge.samples))
                .collect::<Vec<_>>(),
            vec![(1, 2, 3), (2, 3, 2), (2, 4, 1)]
        );
        assert_eq!(profile.folded().len(), 2);
        assert_eq!(profile.folded()[0].samples, 2);
    }

    #[test]
    fn rejects_an_empty_stack() {
        assert_eq!(Sample::new(Vec::new()), Err(SampleError::EmptyStack));
    }

    #[test]
    fn sample_errors_have_stable_messages() {
        for error in [
            SampleError::EmptyStack,
            SampleError::InvalidSamplerCapacity,
            SampleError::MissingReturnPc,
            SampleError::UnknownCodeAddress,
            SampleError::UnknownSafepoint,
            SampleError::EmptyFrameName,
            SampleError::TooManyFrames,
        ] {
            assert!(!error.to_string().is_empty());
        }
    }
}
