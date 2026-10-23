mod budget;
mod vector;
pub use budget::{MetadataBudget, MetadataCharge, MetadataError};
pub use vector::MetadataVec;

#[cfg(test)]
#[path = "tests/metadata_vec.rs"]
mod tests;
