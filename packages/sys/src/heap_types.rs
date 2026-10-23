use crate::Word;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Allocation and registration failures reported by the heap interface.
pub enum StorageCondition {
    /// The configured dynamic space cannot satisfy the allocation.
    CapacityExceeded,
    /// The configured collector metadata budget cannot satisfy the operation.
    HeapMetadataCapacityExceeded,
    /// The allocation coordinator observed collection preparation while holding its gate.
    CollectionInProgress,
    /// The requested object size cannot be represented or is outside the heap contract.
    InvalidSize,
    /// An operation requiring a mutator was attempted by an unregistered thread.
    ThreadNotRegistered,
    /// The allocator returned a heap address in the immediate-value range.
    HeapAddressTooLow,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The storage class used to choose object layout and collection policy.
pub enum PageKind {
    /// A two-word cons cell.
    Cons,
    /// A header-bearing ordinary object.
    HeaderObject,
    /// An object large enough for the large-object policy.
    Large,
    /// Non-moving executable code storage.
    Code,
    /// Runtime-initialized immutable storage.
    Static,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The widetag placed in the header of a heap object.
pub struct TypeTag {
    /// The low-byte object kind consumed by the collector.
    pub widetag: u8,
}
#[derive(Clone, Debug, Eq, PartialEq)]
/// Describes which words of an object contain movable references.
pub struct ReferenceLayout {
    /// Header-inclusive raw indices. Payload `N` is index `N + 1`.
    pub reference_words: Vec<usize>,
    /// Header-inclusive index from which every word to the object end is boxed.
    pub boxed_from: Option<usize>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Failure while registering a widetag layout.
pub enum LayoutError {
    /// The widetag already has a registered layout.
    AlreadyRegistered,
    /// The collector metadata limit cannot reserve layout capacity.
    MetadataCapacityExceeded,
}
#[derive(Clone, Copy, Debug)]
/// Capacity and scheduling limits used when constructing a heap.
pub struct HeapConfig {
    /// Maximum bytes available to moving heap objects.
    pub dynamic_space_size: usize,
    /// Allocation debt that requests the next collection.
    pub bytes_considered_between_gcs: usize,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Optional collection settings used when constructing a heap.
pub struct HeapOptions {
    /// Whether allocation slow paths may request automatic collection.
    pub auto_gc: bool,
    /// Maximum bytes reserved for collector-owned metadata, if configured.
    pub max_metadata_bytes: Option<usize>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Which component of a weak object is retained when its referent dies.
pub enum Weakness {
    /// Retain the value while allowing the key to disappear.
    Key,
    /// Retain the key while allowing the value to disappear.
    Value,
    /// Clear either component independently when its referent dies.
    KeyAndValue,
    /// Retain the pair when either component remains live.
    KeyOrValue,
}
/// Callback invoked after a finalized object is made pending.
pub type Finalizer = fn(Word);
