use super::super::{HeapWords, PageKind};
use crate::heap::{LOGICAL_PAGE_SIZE, PageError, PageRange};
use core::ptr::NonNull;
pub(super) fn allocate_copy(
    state: &mut super::super::State,
    kind: PageKind,
    generation: u8,
    source: *const u64,
    words: usize,
) -> Option<(usize, HeapWords, usize, usize)> {
    let bytes = words.checked_mul(8)?;
    if bytes == 0 || bytes >= LOGICAL_PAGE_SIZE {
        return None;
    }
    let page = state.pages.iter().position(|metadata| {
        metadata.evacuation
            && metadata.kind == Some(kind)
            && metadata.generation == generation
            && metadata
                .used_bytes
                .checked_add(bytes)
                .is_some_and(|end| end <= LOGICAL_PAGE_SIZE)
    });
    let page = if let Some(page) = page {
        page
    } else {
        let page = state.pages.iter().position(|metadata| {
            metadata.kind.is_none()
                && metadata.owner.is_none()
                && metadata.extent_owner.is_none()
                && !metadata.pinned
        })?;
        state.retire_page_forwardings(page);
        let range = state.page_range.as_mut()?;
        range.commit_page(page).ok()?;
        range.clear_page(page).ok()?;
        let metadata = state.pages.get_mut(page)?;
        metadata.reset();
        metadata.kind = Some(kind);
        metadata.generation = generation;
        metadata.evacuation = true;
        page
    };
    let offset = state.pages.get(page)?.used_bytes;
    let range = state.page_range.as_ref()?;
    let page_base = range.page_words_pointer(page)?;
    let word_offset = offset / core::mem::size_of::<u64>();
    // SAFETY: the word offset and copied length fit in this committed evacuation page.
    let destination = unsafe { page_base.as_ptr().add(word_offset) };
    // SAFETY: source is a live object range, destination is a disjoint committed range.
    unsafe { core::ptr::copy_nonoverlapping(source, destination, words) };
    let pointer = NonNull::new(destination)?;
    // SAFETY: destination contains the copied words and the page remains committed in State.
    let managed = unsafe { HeapWords::new(pointer, words) };
    let start = destination.addr();
    state.pages.get_mut(page)?.used_bytes = offset.checked_add(bytes)?;
    Some((start, managed, page, offset))
}

pub(super) fn reclaim_empty_pages(
    state: &mut super::super::State,
    mut decommit_group: impl FnMut(&mut PageRange, usize) -> Result<(), PageError>,
) -> Result<(), PageError> {
    let Some(range) = state.page_range.as_ref() else {
        return Ok(());
    };
    let base = range.base_address();
    let group_count = range.group_count();
    let mut reclaimable = vec![false; state.pages.len()];
    let objects = &state.objects;
    for (page_index, metadata) in state.pages.iter_mut().enumerate() {
        let live_extent = metadata
            .extent_owner
            .is_some_and(|owner| objects.get(owner).is_some_and(|object| object.alive));
        let live_start = metadata
            .object_starts
            .iter()
            .any(|(_, index)| objects.get(*index).is_some_and(|object| object.alive));
        if metadata.pinned || live_extent || live_start {
            metadata.object_starts.retain(|(_, index)| {
                objects
                    .get(*index)
                    .is_some_and(|object| object.alive || object.forwarded_to.is_some())
            });
            continue;
        }
        let page = reclaimable
            .get_mut(page_index)
            .ok_or(PageError::PageOutOfBounds)?;
        *page = true;
    }

    for group in 0..group_count {
        let Some(pages) = state
            .page_range
            .as_ref()
            .and_then(|range| range.pages_in_group(group))
        else {
            continue;
        };
        let group_pages = pages.collect::<Vec<_>>();
        let whole_group_reclaimable = group_pages
            .iter()
            .try_fold(true, |whole, page| {
                reclaimable
                    .get(*page)
                    .map(|page_reclaimable| whole && *page_reclaimable)
            })
            .ok_or(PageError::PageOutOfBounds)?;
        if whole_group_reclaimable {
            decommit_group(
                state
                    .page_range
                    .as_mut()
                    .ok_or(PageError::PageOutOfBounds)?,
                group,
            )?;
            for object in &mut state.objects {
                if object.alive || object.words.is_empty() {
                    continue;
                }
                let Some(offset) = object.start.checked_sub(base) else {
                    continue;
                };
                if group_pages.contains(&(offset / LOGICAL_PAGE_SIZE)) {
                    object.words.retire();
                }
            }
        }
        for page_index in group_pages {
            let is_reclaimable = reclaimable
                .get(page_index)
                .copied()
                .ok_or(PageError::PageOutOfBounds)?;
            if is_reclaimable {
                let metadata = state
                    .pages
                    .get_mut(page_index)
                    .ok_or(PageError::PageOutOfBounds)?;
                metadata.kind = None;
                metadata.generation = 0;
                metadata.pinned = false;
                metadata.used_bytes = 0;
                metadata.owner = None;
                metadata.extent_owner = None;
                metadata.evacuation = false;
                metadata.object_starts.retain(|(_, index)| {
                    state
                        .objects
                        .get(*index)
                        .is_some_and(|object| object.forwarded_to.is_some())
                });
            }
        }
    }
    Ok(())
}
