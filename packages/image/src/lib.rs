//! Image save and load for the NCL runtime.
//!
//! An image captures a rooted object graph, the symbol table, the runtime's
//! feature list, and any published code blocks, so a later process can rebuild
//! them. Saving walks the graph reachable from the caller's roots and records
//! each object once; loading rebuilds the graph in a fresh [`Runtime`] and
//! re-interns symbols into their home packages.
//!
//! [`Runtime`]: ncl_object::Runtime
//!
//! # Format
//!
//! The container is little-endian and versioned by [`FORMAT_VERSION`]. A fixed
//! 64-byte header names the architecture (reusing [`ncl_objfile::Architecture`])
//! and the payload counts; the payload holds one record per reachable object,
//! one reference per root, one blob per code block, and one string per runtime
//! feature. References are either immediate tagged words stored by raw bits or
//! indices into the record table.
//!
//! The `ncl-objfile` FASL container is not reused: its six sections are fixed to
//! code, relocations, constants, symbols, stack maps, and debug records, and its
//! relocations are code-oriented. A heap image needs a variable object table
//! with index references, so the container is defined here and only the
//! architecture model is shared.
//!
//! # Scope and gaps
//!
//! Phase 1 supports cons, symbol, package, string, simple vector, specialized
//! array, hash table, structure, instance, simple function, closure, code
//! object, bignum, ratio, double float, and complex objects. Non-simple arrays,
//! readtables, and streams are rejected with [`ImageError::UnsupportedKind`].
//!
//! The following require `ncl-sys` or `ncl-object` API that is not yet public,
//! so they are out of scope:
//!
//! - Enumerating live heap pages or objects: `State.objects` and `Heap`'s page
//!   accessors are crate-private, so an image is rooted at caller-supplied
//!   values rather than a whole-heap dump.
//! - Reading an object's total word count: the image derives sizes from each
//!   object's kind instead of reading the heap record.
//! - Pausing the world without collecting: the only public stop-the-world entry
//!   point is `ncl_sys::collect`, which runs a collection.
//! - Enumerating the runtime's function, class, and package registries, so only
//!   the feature list is restored as registry state.
//! - Relocating code constant slots: code blocks are saved and republished by
//!   bytes, but a raw entry address is not rewritten to the new allocation.

mod code;
mod error;
mod format;
mod record;
mod register;
mod restore;
mod save;

pub use code::CodeImage;
pub use error::ImageError;
pub use register::register;
pub use restore::{LoadedImage, load};
pub use save::save;

/// Image format version written and accepted by this build.
pub const FORMAT_VERSION: u16 = format::FORMAT_VERSION;
