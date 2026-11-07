//! Form-local source locations and expansion provenance.

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::arena::Arena;
use crate::ids::{ArenaIndex, CompilationUnitId, FormId, IdError, SourceOriginId, SymbolId};

/// Identity of a source file in one compilation unit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceFileId {
    compilation_unit: CompilationUnitId,
    index: u32,
}

impl SourceFileId {
    /// Creates a file identity after checking that its index fits in `u32`.
    ///
    /// # Errors
    ///
    /// Returns `IdError::IndexOverflow` when `index` does not fit in `u32`.
    pub fn try_from_index(
        compilation_unit: CompilationUnitId,
        index: usize,
    ) -> Result<Self, IdError> {
        u32::try_from(index)
            .map(|index| Self {
                compilation_unit,
                index,
            })
            .map_err(|_| IdError::IndexOverflow { index })
    }

    /// Returns the owning compilation unit.
    #[must_use]
    pub const fn compilation_unit(self) -> CompilationUnitId {
        self.compilation_unit
    }

    /// Returns this file's index in its compilation unit.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
}

impl ArenaIndex for SourceFileId {
    type Owner = CompilationUnitId;

    fn owner(self) -> Self::Owner {
        self.compilation_unit
    }

    fn index(self) -> u32 {
        self.index
    }

    fn try_from_index(owner: Self::Owner, index: usize) -> Result<Self, IdError> {
        Self::try_from_index(owner, index)
    }
}

/// A one-based line and byte column at a zero-based byte offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    byte_offset: usize,
    line: usize,
    column: usize,
}

impl SourcePosition {
    /// Returns the zero-based byte offset.
    #[must_use]
    pub const fn byte_offset(self) -> usize {
        self.byte_offset
    }

    /// Returns the one-based line number.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns the one-based byte column.
    #[must_use]
    pub const fn column(self) -> usize {
        self.column
    }
}

/// A validated half-open byte range in a source file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    start: SourcePosition,
    end: SourcePosition,
}

impl SourceSpan {
    /// Returns the first included source position.
    #[must_use]
    pub const fn start(self) -> SourcePosition {
        self.start
    }
    /// Returns the first excluded source position.
    #[must_use]
    pub const fn end(self) -> SourcePosition {
        self.end
    }
}

/// Source file metadata needed to validate byte offsets and resolve positions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    name: String,
    byte_len: usize,
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// Returns the registered display name for this file.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the source length in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    fn from_source(name: String, source: &str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self {
            name,
            byte_len: source.len(),
            line_starts,
        }
    }

    fn position(&self, byte_offset: usize) -> Result<SourcePosition, SourceError> {
        if byte_offset > self.byte_len {
            return Err(SourceError::OffsetOutOfBounds {
                offset: byte_offset,
                length: self.byte_len,
            });
        }
        let line_index = self
            .line_starts
            .partition_point(|line_start| *line_start <= byte_offset)
            .saturating_sub(1);
        let line_start = self
            .line_starts
            .get(line_index)
            .copied()
            .ok_or(SourceError::InvalidSourceFile)?;
        Ok(SourcePosition {
            byte_offset,
            line: line_index + 1,
            column: byte_offset - line_start + 1,
        })
    }

    fn span(&self, start: usize, end: usize) -> Result<SourceSpan, SourceError> {
        if start > end {
            return Err(SourceError::ReversedSpan { start, end });
        }
        Ok(SourceSpan {
            start: self.position(start)?,
            end: self.position(end)?,
        })
    }
}

/// Compilation-unit file registry used to validate source locations.
#[derive(Debug)]
pub struct SourceFiles {
    files: Arena<SourceFile, SourceFileId>,
}

impl SourceFiles {
    /// Creates a registry for one compilation unit.
    #[must_use]
    pub fn new(compilation_unit: CompilationUnitId) -> Self {
        Self {
            files: Arena::new(compilation_unit),
        }
    }

    /// Adds a file and records the byte offsets of its lines.
    ///
    /// # Errors
    ///
    /// Returns `SourceError::Id` if the file index space is exhausted.
    pub fn add(
        &mut self,
        name: impl Into<String>,
        source: &str,
    ) -> Result<SourceFileId, SourceError> {
        self.files
            .allocate(SourceFile::from_source(name.into(), source))
            .map_err(SourceError::Id)
    }

    /// Looks up a file when its identity belongs to this registry.
    #[must_use]
    pub fn get(&self, file: SourceFileId) -> Option<&SourceFile> {
        self.files.get(file)
    }

    /// Constructs a span and its line/column coordinates for `file`.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown file, reversed range, or offset past EOF.
    pub fn span(
        &self,
        file: SourceFileId,
        start: usize,
        end: usize,
    ) -> Result<SourceSpan, SourceError> {
        self.get(file)
            .ok_or(SourceError::UnknownFile(file))?
            .span(start, end)
    }
}

/// The class of transformation that produced an expanded source form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SourceTransformKind {
    /// Expansion of an ordinary macro.
    MacroExpansion,
    /// Expansion of a compiler macro.
    CompilerMacroExpansion,
    /// Another named compiler transformation.
    CompilerTransform,
}

/// An expansion transform, identified by kind and compilation-unit symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTransform {
    kind: SourceTransformKind,
    name: SymbolId,
}

impl SourceTransform {
    /// Creates a transform identity.
    #[must_use]
    pub const fn new(kind: SourceTransformKind, name: SymbolId) -> Self {
        Self { kind, name }
    }

    /// Returns this transform's class.
    #[must_use]
    pub const fn kind(self) -> SourceTransformKind {
        self.kind
    }

    /// Returns the symbol naming this transform.
    #[must_use]
    pub const fn name(self) -> SymbolId {
        self.name
    }
}

/// A file, form, and validated span for one source origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    file: SourceFileId,
    form: FormId,
    span: SourceSpan,
}

impl SourceLocation {
    /// Returns the source file identity.
    #[must_use]
    pub const fn file(self) -> SourceFileId {
        self.file
    }

    /// Returns the owning form identity.
    #[must_use]
    pub const fn form(self) -> FormId {
        self.form
    }

    /// Returns the validated source range and coordinates.
    #[must_use]
    pub const fn span(self) -> SourceSpan {
        self.span
    }
}

/// The provenance kind of a source-derived HIR operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SourceOriginKind {
    /// A location directly present in source text.
    Source,
    /// A generated location linked to its expansion parent.
    Expansion {
        /// The origin from which this expansion was produced.
        parent: SourceOriginId,
        /// The named transformation that produced this location.
        transform: SourceTransform,
    },
}

/// One form-local source location or generated expansion record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceOrigin {
    id: SourceOriginId,
    location: SourceLocation,
    kind: SourceOriginKind,
}

impl SourceOrigin {
    /// Returns this origin's location.
    #[must_use]
    pub const fn location(self) -> SourceLocation {
        self.location
    }

    /// Returns whether this is source text or generated expansion output.
    #[must_use]
    pub const fn kind(self) -> SourceOriginKind {
        self.kind
    }
}

/// Form-local origins referenced by HIR nodes.
#[derive(Debug)]
pub struct SourceOrigins {
    form: FormId,
    origins: Vec<SourceOrigin>,
}

impl SourceOrigins {
    /// Creates an empty origin table for one top-level form.
    #[must_use]
    pub fn new(form: FormId) -> Self {
        Self {
            form,
            origins: Vec::new(),
        }
    }

    /// Returns the form owning these source origins.
    #[must_use]
    pub const fn form(&self) -> FormId {
        self.form
    }

    /// Adds a location that directly comes from source text.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid file/span or exhausted origin IDs.
    pub fn add_source(
        &mut self,
        files: &SourceFiles,
        file: SourceFileId,
        span: SourceSpan,
    ) -> Result<SourceOriginId, SourceError> {
        let location = self.location(files, file, span)?;
        self.allocate(SourceOriginKind::Source, location)
    }

    /// Adds a generated location linked to an existing parent origin.
    ///
    /// Parents must already exist, so expansion-origin cycles cannot be formed.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid parent, transform, file/span, or exhausted
    /// origin IDs.
    pub fn add_expansion(
        &mut self,
        files: &SourceFiles,
        file: SourceFileId,
        span: SourceSpan,
        parent: SourceOriginId,
        transform: SourceTransform,
    ) -> Result<SourceOriginId, SourceError> {
        self.get(parent).ok_or(SourceError::UnknownOrigin(parent))?;
        if transform.name.compilation_unit() != self.form.compilation_unit() {
            return Err(SourceError::ForeignTransform(transform.name));
        }
        let location = self.location(files, file, span)?;
        self.allocate(SourceOriginKind::Expansion { parent, transform }, location)
    }

    /// Looks up an origin when it belongs to this form and exists.
    #[must_use]
    pub fn get(&self, origin: SourceOriginId) -> Option<&SourceOrigin> {
        if origin.form() != self.form {
            return None;
        }
        let index = usize::try_from(origin.index()).ok()?;
        self.origins.get(index).filter(|stored| stored.id == origin)
    }

    /// Iterates over allocated origin identities and records in allocation order.
    #[must_use]
    pub fn iter(&self) -> impl Iterator<Item = (SourceOriginId, &SourceOrigin)> {
        self.origins.iter().map(|origin| (origin.id, origin))
    }

    /// Returns the number of stored source origins.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.origins.len()
    }

    /// Returns whether the table contains any source origins.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }

    fn location(
        &self,
        files: &SourceFiles,
        file: SourceFileId,
        span: SourceSpan,
    ) -> Result<SourceLocation, SourceError> {
        if file.compilation_unit() != self.form.compilation_unit() {
            return Err(SourceError::ForeignFile(file));
        }
        let expected = files.span(file, span.start.byte_offset, span.end.byte_offset)?;
        if expected != span {
            return Err(SourceError::InvalidCoordinates);
        }
        Ok(SourceLocation {
            file,
            form: self.form,
            span,
        })
    }

    fn allocate(
        &mut self,
        kind: SourceOriginKind,
        location: SourceLocation,
    ) -> Result<SourceOriginId, SourceError> {
        let id = SourceOriginId::try_from_index(self.form, self.origins.len())
            .map_err(SourceError::Id)?;
        self.origins.push(SourceOrigin { id, location, kind });
        Ok(id)
    }
}

/// Errors from source-file, span, and origin construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SourceError {
    /// An arena identity index exceeded the supported range.
    Id(IdError),
    /// A file identity belongs to a different compilation unit.
    ForeignFile(SourceFileId),
    /// No registered file matches this identity.
    UnknownFile(SourceFileId),
    /// A transform symbol belongs to a different compilation unit.
    ForeignTransform(SymbolId),
    /// An origin identity is outside this form's table or out of bounds.
    UnknownOrigin(SourceOriginId),
    /// The source offsets and line/column coordinates do not match the file.
    InvalidCoordinates,
    /// A source range has its end before its start.
    ReversedSpan {
        /// The byte offset at the start of the range.
        start: usize,
        /// The byte offset at the end of the range.
        end: usize,
    },
    /// A source offset is past the file's end.
    OffsetOutOfBounds {
        /// The requested byte offset.
        offset: usize,
        /// The source file's byte length.
        length: usize,
    },
    /// The file line table is internally invalid.
    InvalidSourceFile,
}

impl Display for SourceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "source error: {self:?}")
    }
}

impl Error for SourceError {}
