//! Reconstruction of a saved object graph into a destination runtime.

#[path = "restore_objects.rs"]
mod restore_objects;
#[path = "restore_refs.rs"]
mod restore_refs;
#[path = "restore_roots.rs"]
mod restore_roots;

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_sys::{CodePtr, RootToken};

use crate::adapter::parse;
use crate::domain::Architecture;
use crate::error::ImageError;

/// Objects restored from an image.
#[derive(Debug)]
pub struct LoadedImage {
    /// Restored root values in the same order they were saved.
    roots: Box<[Word]>,
    /// Republished code blocks in the same order they were saved.
    pub code: Vec<CodePtr>,
    root_token: RootToken,
}

impl LoadedImage {
    /// Borrow roots while this image owns their precise root registration.
    #[must_use]
    pub fn roots(&self) -> &[Word] { &self.roots }

    /// Release the precise roots owned by this image.
    ///
    /// The image must be released in reverse order of other root registrations
    /// made on the same thread. Keeping the image alive keeps its roots valid.
    ///
    /// # Errors
    /// Returns an error when another root was registered after this image.
    pub fn release(self, ctx: &mut ThreadContext) -> Result<(), ImageError> {
        if ncl_sys::pop_root(ctx.thread_mut(), self.root_token) {
            Ok(())
        } else {
            Err(ImageError::InvalidField { field: "root ownership" })
        }
    }
}

/// Restore an image into `runtime`, returning its roots and code blocks.
///
/// Symbols are re-interned into their saved packages, so loading twice yields
/// the same symbols. Code blocks are republished into fresh, non-moving
/// allocations; the returned pointers own those allocations.
///
/// # Errors
/// Returns [`ImageError`] when the byte stream is malformed, targets another
/// architecture, references an unknown record, or allocation fails.
#[must_use]
pub fn load(bytes: &[u8], runtime: &Runtime, ctx: &mut ThreadContext) -> Result<LoadedImage, ImageError> {
    let file = parse(bytes)?;
    check_architecture(file.architecture)?;
    let roots = restore_roots::with_slots(ctx, file.objects.len(), |ctx, slots| {
        restore_objects::rebuild(runtime, ctx, &file, slots)
    })?;
    let code = file.code.iter().map(|image| image.publish()).collect::<Result<Vec<_>, _>>()?;
    let mut roots = roots.into_boxed_slice();
    let root_token = restore_roots::register(ctx, &mut roots);
    Ok(LoadedImage { roots, code, root_token })
}

fn check_architecture(architecture: Architecture) -> Result<(), ImageError> {
    if architecture == Architecture::host() { Ok(()) } else { Err(ImageError::InvalidField { field: "architecture" }) }
}
