use super::{PageMetadata, State};
use crate::heap::LOGICAL_PAGE_SIZE;

const OBJECT_WORDS_PER_PAGE: usize = LOGICAL_PAGE_SIZE / std::mem::size_of::<u64>();
const BITS_PER_BITMAP_WORD: usize = 64;
const BITMAP_WORDS: usize = OBJECT_WORDS_PER_PAGE.div_ceil(BITS_PER_BITMAP_WORD);
const RANK_WORDS: usize = 8;
const RANK_GROUPS: usize = BITMAP_WORDS.div_ceil(RANK_WORDS);

#[derive(Debug)]
pub(super) struct ObjectStartBitmap {
    bits: [u64; BITMAP_WORDS],
    word_counts: [u8; BITMAP_WORDS],
    group_counts: [u16; RANK_GROUPS],
}

impl Default for ObjectStartBitmap {
    fn default() -> Self {
        Self {
            bits: [0; BITMAP_WORDS],
            word_counts: [0; BITMAP_WORDS],
            group_counts: [0; RANK_GROUPS],
        }
    }
}

impl ObjectStartBitmap {
    fn insert(&mut self, byte_offset: usize) {
        if !byte_offset.is_multiple_of(std::mem::size_of::<u64>()) {
            return;
        }
        let word_offset = byte_offset / std::mem::size_of::<u64>();
        if word_offset >= OBJECT_WORDS_PER_PAGE {
            return;
        }
        let bitmap_word = word_offset / BITS_PER_BITMAP_WORD;
        let bit = 1_u64 << (word_offset % BITS_PER_BITMAP_WORD);
        let Some(bits) = self.bits.get_mut(bitmap_word) else {
            return;
        };
        if *bits & bit != 0 {
            return;
        }
        *bits |= bit;
        if let Some(count) = self.word_counts.get_mut(bitmap_word) {
            *count += 1;
        }
        let group = bitmap_word / RANK_WORDS;
        if let Some(count) = self.group_counts.get_mut(group) {
            *count += 1;
        }
    }

    fn rank_of(&self, byte_offset: usize) -> Option<usize> {
        if !byte_offset.is_multiple_of(std::mem::size_of::<u64>()) {
            return None;
        }
        let word_offset = byte_offset / std::mem::size_of::<u64>();
        if word_offset >= OBJECT_WORDS_PER_PAGE {
            return None;
        }
        let bitmap_word = word_offset / BITS_PER_BITMAP_WORD;
        let bit_offset = word_offset % BITS_PER_BITMAP_WORD;
        let group = bitmap_word / RANK_WORDS;
        let first_word = group * RANK_WORDS;
        let preceding_groups = self.group_counts.get(..group)?.iter().copied();
        let preceding_words = self
            .word_counts
            .get(first_word..bitmap_word)?
            .iter()
            .copied();
        let bits = *self.bits.get(bitmap_word)?;
        let bit = 1_u64 << bit_offset;
        if bits & bit == 0 {
            return None;
        }
        let before_bit = if bit_offset == 0 { 0 } else { bit - 1 };
        let preceding_bits = usize::try_from((bits & before_bit).count_ones()).ok()?;
        Some(
            preceding_groups.map(usize::from).sum::<usize>()
                + preceding_words.map(usize::from).sum::<usize>()
                + preceding_bits,
        )
    }
}

impl PageMetadata {
    pub(crate) fn clear_object_starts(&mut self) {
        self.object_starts.clear();
        self.object_start_bitmap = None;
    }

    pub(crate) fn add_object_start(&mut self, offset: usize, index: usize) {
        match self
            .object_starts
            .binary_search_by_key(&offset, |(start, _)| *start)
        {
            Ok(position) => {
                if let Some((_, existing_index)) = self.object_starts.get_mut(position) {
                    *existing_index = index;
                }
            }
            Err(position) => self.object_starts.insert(position, (offset, index)),
        }
        self.object_start_bitmap
            .get_or_insert_with(Box::default)
            .insert(offset);
    }

    pub(crate) fn rebuild_object_start_bitmap(&mut self) {
        if self.object_starts.is_empty() {
            self.object_start_bitmap = None;
            return;
        }
        let mut bitmap = Box::<ObjectStartBitmap>::default();
        for (offset, _) in &self.object_starts {
            bitmap.insert(*offset);
        }
        self.object_start_bitmap = Some(bitmap);
    }
}

impl State {
    pub(crate) fn object_index_at(&self, address: usize) -> Option<usize> {
        let range = self.page_range.as_ref()?;
        let page = self.page_index(address)?;
        let page_base = range.page_address(page)?;
        let offset = address.checked_sub(page_base)?;
        let metadata = self.pages.get(page)?;
        let position = metadata.object_start_bitmap.as_ref()?.rank_of(offset)?;
        metadata
            .object_starts
            .get(position)
            .and_then(|(start, index)| (*start == offset).then_some(*index))
    }

    pub(crate) fn rebuild_object_starts(&mut self) {
        for page in &mut self.pages {
            page.clear_object_starts();
            page.incoming_forwarders.clear();
        }
        let Some(range) = self.page_range.as_ref() else {
            return;
        };
        let base = range.base_address();
        let mut starts = Vec::new();
        let mut incoming_forwarders = Vec::new();
        for (index, object) in self.objects.iter().enumerate() {
            if !object.alive && object.forwarded_to.is_none() {
                continue;
            }
            let Some(offset) = object.start.checked_sub(base) else {
                continue;
            };
            let page = offset / LOGICAL_PAGE_SIZE;
            if page < self.pages.len() {
                starts.push((page, offset % LOGICAL_PAGE_SIZE, index));
            }
            if let Some(target_index) = object.forwarded_to
                && let Some(target) = self.objects.get(target_index)
                && let Some(target_offset) = target.start.checked_sub(base)
            {
                let target_page = target_offset / LOGICAL_PAGE_SIZE;
                if target_page < self.pages.len() {
                    incoming_forwarders.push((target_page, index));
                }
            }
        }
        starts.sort_unstable_by_key(|(page, offset, _)| (*page, *offset));
        for (page, offset, index) in starts {
            if let Some(page) = self.pages.get_mut(page) {
                page.add_object_start(offset, index);
            }
        }
        for (page, source_index) in incoming_forwarders {
            if let Some(page) = self.pages.get_mut(page) {
                page.incoming_forwarders.push(source_index);
            }
        }
    }
}
