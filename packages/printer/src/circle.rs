//! Shared-structure detection for `*print-circle*`.

use std::collections::{HashMap, HashSet};

use ncl_object::{
    ObjectRef, ThreadContext, Word, array_dimensions, array_row_major_ref, car, cdr,
    classify_object, simple_vector_length, simple_vector_ref, specialized_array_ref,
};

/// The largest specialized array the length probe will walk.
const MAX_PROBE: usize = 1_000_000;

/// A label the pre-scan assigned to a shared object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CircleLabel {
    /// The first occurrence, printed as `#n=`.
    Definition(usize),
    /// A later occurrence, printed as `#n#`.
    Reference(usize),
}

/// The labels for one print operation, keyed by object address.
#[derive(Debug, Default)]
pub struct CircleState {
    labels: HashMap<usize, usize>,
    emitted: HashSet<usize>,
}

impl CircleState {
    /// Scan `root` and label every shared object in first-visit order.
    ///
    /// When `not_shared` is true, only objects that lie on a cycle are
    /// labelled, matching `NCL-EXT:*PRINT-CIRCLE-NOT-SHARED*`.
    ///
    /// The scan reads object fields only, so it never allocates on the Lisp
    /// heap and the addresses it records stay valid until printing finishes.
    pub fn scan(ctx: &mut ThreadContext, root: Word, not_shared: bool) -> Self {
        let mut counts = HashMap::new();
        let mut visited = HashSet::new();
        count_references(ctx, root, &mut counts, &mut visited);
        let mut labels = HashMap::new();
        let mut next = 1usize;
        let mut assigned = HashSet::new();
        assign_labels(
            ctx,
            root,
            &counts,
            not_shared,
            &mut labels,
            &mut next,
            &mut assigned,
        );
        Self {
            labels,
            emitted: HashSet::new(),
        }
    }

    /// Classify an address, or `None` when it carries no label.
    pub fn enter(&mut self, address: usize) -> Option<CircleLabel> {
        let label = *self.labels.get(&address)?;
        if self.emitted.insert(address) {
            Some(CircleLabel::Definition(label))
        } else {
            Some(CircleLabel::Reference(label))
        }
    }

    /// Whether an address carries a label.
    pub fn has_label(&self, address: usize) -> bool {
        self.labels.contains_key(&address)
    }
}

/// Whether an object is traversed and can carry a circle label.
///
/// Conses are detected through their lowtag: `classify_object` reads the
/// widetag from the first payload word, which a headerless cons does not have.
/// Characters are excluded because `Word::character` shares the `List` lowtag.
/// Structures and instances print as opaque `#<...>` forms without recursing
/// into their slots, so they cannot take part in a printed cycle.
pub fn labelable(ctx: &ThreadContext, object: Word) -> bool {
    if crate::print::character_code(object).is_some() {
        return false;
    }
    object.is_cons()
        || matches!(
            classify_object(ctx, object),
            ObjectRef::SimpleVector(_) | ObjectRef::SpecializedArray(_) | ObjectRef::Array(_)
        )
}

fn count_references(
    ctx: &mut ThreadContext,
    root: Word,
    counts: &mut HashMap<usize, usize>,
    visited: &mut HashSet<usize>,
) {
    let mut stack = vec![root];
    while let Some(object) = stack.pop() {
        if !labelable(ctx, object) {
            continue;
        }
        let address = object.address();
        *counts.entry(address).or_insert(0) += 1;
        if !visited.insert(address) {
            continue;
        }
        stack.extend(children(ctx, object));
    }
}

fn assign_labels(
    ctx: &mut ThreadContext,
    root: Word,
    counts: &HashMap<usize, usize>,
    not_shared: bool,
    labels: &mut HashMap<usize, usize>,
    next: &mut usize,
    assigned: &mut HashSet<usize>,
) {
    let mut stack = vec![root];
    while let Some(object) = stack.pop() {
        if !labelable(ctx, object) {
            continue;
        }
        let address = object.address();
        if !assigned.insert(address) {
            continue;
        }
        let count = counts.get(&address).copied().unwrap_or(0);
        if count > 1 && (!not_shared || reaches_itself(ctx, object)) {
            labels.insert(address, *next);
            *next += 1;
        }
        let mut pending = children(ctx, object);
        pending.reverse();
        stack.extend(pending);
    }
}

/// Whether `start` is reachable from one of its own children.
fn reaches_itself(ctx: &mut ThreadContext, start: Word) -> bool {
    let target = start.address();
    let mut seen = HashSet::new();
    let mut stack = children(ctx, start);
    while let Some(object) = stack.pop() {
        if !labelable(ctx, object) {
            continue;
        }
        let address = object.address();
        if address == target {
            return true;
        }
        if !seen.insert(address) {
            continue;
        }
        stack.extend(children(ctx, object));
    }
    false
}

/// The immediate children of a labelable object.
///
/// A cons contributes its `car` and `cdr` as two separate graph edges, so a
/// cycle through either field is counted once per reference.
fn children(ctx: &mut ThreadContext, object: Word) -> Vec<Word> {
    let mut out = Vec::new();
    if object.is_cons() {
        if let Ok(head) = car(ctx, object) {
            out.push(head);
        }
        if let Ok(tail) = cdr(ctx, object) {
            out.push(tail);
        }
        return out;
    }
    let classified = classify_object(ctx, object);
    if let ObjectRef::SimpleVector(vector) = classified {
        if let Ok(length) = simple_vector_length(ctx, vector) {
            for index in 0..length {
                if let Ok(value) = simple_vector_ref(ctx, vector, index) {
                    out.push(value);
                }
            }
        }
    } else if let ObjectRef::SpecializedArray(array) = classified {
        for index in 0..specialized_length(ctx, array) {
            if let Ok(value) = specialized_array_ref(ctx, array, index) {
                out.push(value);
            }
        }
    } else if let ObjectRef::Array(array) = classified
        && let Some(total) = array_total(ctx, array)
    {
        for index in 0..total {
            if let Ok(value) = array_row_major_ref(ctx, array, index) {
                out.push(value);
            }
        }
    }
    out
}

/// Probe a specialized array's length.
///
/// `ncl-object` exposes no specialized-array length accessor, so the length is
/// found by reading elements until `specialized_array_ref` reports the index is
/// past the end.
pub fn specialized_length(ctx: &ThreadContext, array: Word) -> usize {
    let mut length = 0usize;
    while length < MAX_PROBE && specialized_array_ref(ctx, array, length).is_ok() {
        length += 1;
    }
    length
}

/// The row-major element count of a non-simple array.
pub fn array_total(ctx: &ThreadContext, array: Word) -> Option<usize> {
    array_dimensions(ctx, array)
        .ok()
        .map(|dimensions| dimensions.iter().product())
}
