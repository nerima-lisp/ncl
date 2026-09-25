//! Record and reference codec for the image payload.
//!
//! Every reachable heap object is encoded as one [`Record`]. References are
//! either immediate tagged words stored by raw bits or indices into the record
//! table, so the object table can be read and written in a single pass.

#[path = "record_code.rs"]
mod record_code;
#[path = "record_header.rs"]
mod record_header;
#[path = "record_refs.rs"]
mod record_refs;
#[path = "record_sections.rs"]
mod record_sections;
#[path = "record_types.rs"]
mod record_types;
#[path = "record_validation.rs"]
mod record_validation;

pub use record_code::{get_code, put_code};
#[allow(unused_imports)]
pub use record_refs::{get_ref, get_refs, put_ref, put_refs, Ref};
pub use record_sections::{get_record, put_record};
pub use record_types::Record;
pub use record_validation::{validate_record, validate_ref};
