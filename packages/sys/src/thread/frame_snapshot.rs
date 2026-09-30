//! Precise capture and write-back of one generated (compiled) native frame,
//! used by the safepoint-poll path to give a collection an exact view of a
//! live compiled caller instead of relying on conservative scanning alone.

use super::Thread;
use crate::Word;

impl Thread {
    pub(crate) fn scan_native_frame(
        &mut self,
        registry: &crate::CodeRegistry,
        forward: impl FnMut(Word) -> Word,
    ) -> Option<usize> {
        let return_pc = usize::try_from(self.frame_chain.get(1)?.bits()).ok()?;
        let (metadata, offset) = registry.find(return_pc)?;
        let map = metadata.safepoint_map.find_map(offset)?;
        crate::scan_frame_with_registers(
            &mut self.frame_chain,
            0,
            map,
            &mut self.frame_registers,
            forward,
        )
    }
    pub(crate) const fn has_native_frame_snapshot(&self) -> bool {
        self.frame_address.is_some()
    }
    /// Install a precise native frame and register snapshot for collection.
    pub fn set_frame_snapshot(&mut self, frames: Vec<Word>, registers: Vec<Word>) {
        self.frame_chain = frames;
        self.frame_registers = registers;
        self.frame_address = None;
        self.frame_snapshot_failed = false;
        self.frame_last_written.clear();
    }
    /// Capture the current generated frame header for collection.
    ///
    /// # Safety
    ///
    /// `frame_fp` must point to a live generated frame whose `frame_words`
    /// words are readable, and `return_pc` must be that frame's continuation PC.
    pub unsafe fn set_native_frame(&mut self, frame_fp: usize, return_pc: usize) {
        let map = self
            .heap()
            .and_then(|heap| heap.safepoint_map_for_pc(return_pc));
        let Some(map) = map else {
            self.frame_snapshot_failed = true;
            self.frame_chain.clear();
            self.frame_registers.clear();
            self.frame_address = None;
            return;
        };
        let frame_words = usize::from(map.frame_words);
        // check-added-lines: allow(as-cast) frame_fp is a raw frame address by contract
        let words = frame_fp as *const Word;
        let mut snapshot = Vec::with_capacity(frame_words);
        // SAFETY: the caller guarantees the generated frame has the mapped width.
        unsafe {
            for index in 0..4 {
                snapshot.push(words.add(index).read());
            }
            for index in 4..frame_words {
                snapshot.push(words.sub(index - 3).read());
            }
        }
        self.frame_chain = snapshot;
        #[cfg(target_arch = "x86_64")]
        {
            self.frame_registers = crate::snapshot_callee_saved()
                .into_iter()
                .map(Word::from_bits)
                .collect();
        }
        self.stack_bounds = None;
        self.callee_saved = [0; 16];
        // check-added-lines: allow(index, as-cast) frame_words is always at least the four-word header
        self.frame_chain[1] = Word::from_bits(return_pc as u64);
        self.frame_address = Some(frame_fp);
        self.frame_snapshot_failed = false;
    }

    /// Capture a callback-provided generated frame.
    pub fn capture_native_frame(&mut self, frame_fp: usize, return_pc: usize) {
        // SAFETY: this entry point is called by the generated safepoint callback with its live frame.
        unsafe { self.set_native_frame(frame_fp, return_pc) };
    }

    /// Return the current value of a captured real frame word.
    #[must_use]
    pub fn frame_word(&self, index: usize) -> Option<Word> {
        self.frame_chain.get(index).copied()
    }

    /// Whether the latest native-frame capture could not resolve its PC map.
    #[must_use]
    pub const fn frame_snapshot_failed(&self) -> bool {
        self.frame_snapshot_failed
    }

    /// Write collection's forwarded snapshot values back to the generated frame.
    pub fn write_back_frame_snapshot(&mut self) {
        let Some(address) = self.frame_address else {
            return;
        };
        // SAFETY: the captured generated frame remains active during collection and write-back.
        unsafe {
            // check-added-lines: allow(as-cast) address is a raw frame address by contract
            let frame = address as *mut Word;
            for (index, word) in self.frame_chain.iter().copied().enumerate() {
                if index == 1 {
                    continue;
                }
                if index < 4 {
                    frame.add(index).write(word);
                } else {
                    frame.sub(index - 3).write(word);
                }
            }
        }
        self.frame_last_written = self.frame_chain.clone();
        self.frame_address = None;
        self.frame_chain.clear();
        self.frame_registers.clear();
    }

    /// Return a word written by the most recent native-frame snapshot.
    #[must_use]
    pub fn last_written_frame_word(&self, index: usize) -> Option<Word> {
        self.frame_last_written.get(index).copied()
    }
}
