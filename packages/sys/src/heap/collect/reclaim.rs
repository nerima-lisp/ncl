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
    let Some((base, group_count)) = state
        .page_range
        .as_ref()
        .map(|range| (range.base_address(), range.group_count()))
    else {
        return Ok(());
    };

    let dead_object_links = dead_object_links(state, base, group_count)?;
    let reclaimable = mark_reclaimable_pages(state)?;

    for group in 0..group_count {
        let Some(pages) = state
            .page_range
            .as_ref()
            .and_then(|range| range.pages_in_group(group))
        else {
            continue;
        };
        if group_is_reclaimable(&pages, &reclaimable)? {
            decommit_group(
                state
                    .page_range
                    .as_mut()
                    .ok_or(PageError::PageOutOfBounds)?,
                group,
            )?;
            retire_group_objects(state, group, &dead_object_links)?;
        }
        reset_reclaimable_pages(state, pages, &reclaimable)?;
    }
    Ok(())
}

struct DeadObjectLinks {
    group_heads: Vec<Option<usize>>,
    next_object: Vec<Option<usize>>,
}

fn dead_object_links(
    state: &super::super::State,
    base: usize,
    group_count: usize,
) -> Result<DeadObjectLinks, PageError> {
    let mut dead_object_heads: Vec<Option<usize>> = Vec::new();
    dead_object_heads
        .try_reserve_exact(group_count)
        .map_err(|_| PageError::MetadataAllocationFailed)?;
    dead_object_heads.resize(group_count, None);
    let mut next_dead_object: Vec<Option<usize>> = Vec::new();
    next_dead_object
        .try_reserve_exact(state.objects.len())
        .map_err(|_| PageError::MetadataAllocationFailed)?;
    next_dead_object.resize(state.objects.len(), None);
    for (object_index, object) in state.objects.iter().enumerate() {
        if object.alive || object.words.is_empty() {
            continue;
        }
        let Some(offset) = object.start.checked_sub(base) else {
            continue;
        };
        let page = offset / LOGICAL_PAGE_SIZE;
        let Some(group) = state
            .page_range
            .as_ref()
            .and_then(|range| range.group_for_page(page))
        else {
            continue;
        };
        let previous = dead_object_heads
            .get(group)
            .copied()
            .ok_or(PageError::GroupOutOfBounds)?;
        *next_dead_object
            .get_mut(object_index)
            .ok_or(PageError::InvalidAllocationRecord)? = previous;
        *dead_object_heads
            .get_mut(group)
            .ok_or(PageError::GroupOutOfBounds)? = Some(object_index);
    }
    Ok(DeadObjectLinks {
        group_heads: dead_object_heads,
        next_object: next_dead_object,
    })
}

fn mark_reclaimable_pages(state: &mut super::super::State) -> Result<Vec<bool>, PageError> {
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
    Ok(reclaimable)
}

fn group_is_reclaimable(
    pages: &core::ops::Range<usize>,
    reclaimable: &[bool],
) -> Result<bool, PageError> {
    let mut whole_group_reclaimable = true;
    for page_index in pages.clone() {
        whole_group_reclaimable &= *reclaimable
            .get(page_index)
            .ok_or(PageError::PageOutOfBounds)?;
    }
    Ok(whole_group_reclaimable)
}

fn retire_group_objects(
    state: &mut super::super::State,
    group: usize,
    dead_object_links: &DeadObjectLinks,
) -> Result<(), PageError> {
    let mut object_index = dead_object_links
        .group_heads
        .get(group)
        .copied()
        .ok_or(PageError::GroupOutOfBounds)?;
    while let Some(index) = object_index {
        object_index = dead_object_links
            .next_object
            .get(index)
            .copied()
            .ok_or(PageError::InvalidAllocationRecord)?;
        state
            .objects
            .get_mut(index)
            .ok_or(PageError::InvalidAllocationRecord)?
            .words
            .retire();
    }
    Ok(())
}

fn reset_reclaimable_pages(
    state: &mut super::super::State,
    pages: core::ops::Range<usize>,
    reclaimable: &[bool],
) -> Result<(), PageError> {
    let objects = &state.objects;
    for page_index in pages {
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
                objects
                    .get(*index)
                    .is_some_and(|object| object.forwarded_to.is_some())
            });
            if metadata.object_starts.is_empty() {
                metadata.incoming_forwarders.clear();
            }
        }
    }
    Ok(())
}
