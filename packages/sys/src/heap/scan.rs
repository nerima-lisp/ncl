use super::{PageKind, WIDETAG_MASK};
use crate::heap::metadata_map::MetadataMap;
use crate::heap_state::LayoutEntry;
use crate::heap_types::ReferenceLayout;
use core::ops::Range;
use core::slice::Iter;

pub(super) struct LayoutSlots<'a> {
    cons_next: u8,
    reference_words: Iter<'a, usize>,
    boxed_words: Option<Range<usize>>,
}

impl Iterator for LayoutSlots<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cons_next < 2 {
            let slot = usize::from(self.cons_next);
            self.cons_next += 1;
            return Some(slot);
        }
        self.reference_words
            .next()
            .copied()
            .or_else(|| self.boxed_words.as_mut().and_then(Iterator::next))
    }
}

pub(super) fn layout(
    layouts: &MetadataMap<u8, LayoutEntry>,
    kind: PageKind,
    header: u64,
    object_words: usize,
) -> LayoutSlots<'_> {
    if kind == PageKind::Cons {
        return LayoutSlots {
            cons_next: 0,
            reference_words: [].iter(),
            boxed_words: None,
        };
    }
    let widetag = u8::try_from(header & WIDETAG_MASK).unwrap_or(0);
    let reference_layout: Option<&ReferenceLayout> =
        layouts.get(&widetag).map(|entry| &entry.layout);
    let Some(reference_layout) = reference_layout else {
        return LayoutSlots {
            cons_next: 2,
            reference_words: [].iter(),
            boxed_words: None,
        };
    };
    LayoutSlots {
        cons_next: 2,
        reference_words: reference_layout.reference_words.iter(),
        boxed_words: reference_layout.boxed_from.map(|start| start..object_words),
    }
}
