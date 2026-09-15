use crate::Word;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageCondition {
    CapacityExceeded,
    InvalidSize,
    ThreadNotRegistered,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageKind {
    Cons,
    HeaderObject,
    Large,
    Code,
    Static,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeTag {
    pub widetag: u8,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceLayout {
    pub reference_words: Vec<usize>,
}
#[derive(Debug)]
pub struct LayoutError;
#[derive(Clone, Copy, Debug)]
pub struct HeapConfig {
    pub dynamic_space_size: usize,
    pub bytes_considered_between_gcs: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Weakness {
    Key,
    Value,
    KeyAndValue,
    KeyOrValue,
}
pub type Finalizer = fn(Word);
