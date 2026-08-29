use std::hash::{BuildHasher, Hasher};

/// A `xorshift64*` pseudo-random number generator backing `RANDOM-STATE`.
#[derive(Clone, Debug)]
pub struct RandomState {
    state: u64,
}

impl RandomState {
    /// Creates a state seeded from the operating system's random source.
    #[must_use]
    pub fn seeded() -> Self {
        let seed = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish();
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// Advances the generator and returns the next 64-bit sample.
    pub const fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod tests {
    use super::RandomState;

    #[test]
    fn zero_state_never_advances() {
        // xorshift64* is fixed at zero forever if seeded with zero, which is
        // exactly why `seeded` maps a zero OS-random draw to a fixed nonzero
        // constant instead.
        let mut state = RandomState { state: 0 };
        assert_eq!(state.next_u64(), 0);
    }

    #[test]
    fn successive_samples_differ() {
        let mut state = RandomState::seeded();
        let first = state.next_u64();
        let second = state.next_u64();
        assert_ne!(first, second);
    }

    #[test]
    fn same_starting_state_reproduces_the_same_sequence() {
        let mut a = RandomState { state: 42 };
        let mut b = RandomState { state: 42 };
        assert_eq!(a.next_u64(), b.next_u64());
        assert_eq!(a.next_u64(), b.next_u64());
    }
}
