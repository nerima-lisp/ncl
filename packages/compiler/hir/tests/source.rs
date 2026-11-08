//! Source file coordinates and expansion-origin identity checks.

use std::error::Error;

use ncl_hir::{
    CompilationUnitId, FormId, SourceError, SourceFileId, SourceFiles, SourceOriginId,
    SourceOriginKind, SourceOrigins, SourceTransform, SourceTransformKind, SymbolId,
};

#[test]
fn source_spans_report_byte_positions_across_lines_and_utf8() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(4);
    let mut files = SourceFiles::new(unit);
    let file = files.add("source.lisp", "λx\ny")?;
    let registered = files.get(file).ok_or(SourceError::UnknownFile(file))?;
    assert_eq!(registered.name(), "source.lisp");
    assert_eq!(registered.byte_len(), 5);
    assert_eq!(file.compilation_unit(), unit);
    assert_eq!(file.index(), 0);

    let span = files.span(file, 2, 5)?;
    assert_eq!(span.start().byte_offset(), 2);
    assert_eq!(span.start().line(), 1);
    assert_eq!(span.start().column(), 3);
    assert_eq!(span.end().byte_offset(), 5);
    assert_eq!(span.end().line(), 2);
    assert_eq!(span.end().column(), 2);
    Ok(())
}

#[test]
fn source_spans_reject_unknown_files_reversed_ranges_and_offsets_past_eof()
-> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(5);
    let mut files = SourceFiles::new(unit);
    let file = files.add("short.lisp", "x")?;
    let unknown = SourceFileId::try_from_index(unit, 1)?;

    assert_eq!(files.get(unknown), None);
    assert_eq!(
        files.span(unknown, 0, 0),
        Err(SourceError::UnknownFile(unknown))
    );
    assert_eq!(
        files.span(file, 1, 0),
        Err(SourceError::ReversedSpan { start: 1, end: 0 })
    );
    assert_eq!(
        files.span(file, 0, 2),
        Err(SourceError::OffsetOutOfBounds {
            offset: 2,
            length: 1,
        })
    );
    assert_eq!(
        SourceFileId::try_from_index(unit, usize::MAX),
        Err(ncl_hir::IdError::IndexOverflow { index: usize::MAX })
    );
    Ok(())
}

#[test]
fn source_origins_preserve_source_and_expansion_provenance() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(6);
    let form = FormId::new(unit, 2);
    let mut files = SourceFiles::new(unit);
    let file = files.add("macro.lisp", "(twice x)")?;
    let span = files.span(file, 0, 9)?;
    let mut origins = SourceOrigins::new(form);
    assert!(origins.is_empty());
    assert_eq!(origins.len(), 0);
    assert_eq!(origins.form(), form);

    let source = origins.add_source(&files, file, span)?;
    let transform_name = SymbolId::try_from_index(unit, 8)?;
    let transform = SourceTransform::new(SourceTransformKind::MacroExpansion, transform_name);
    let expansion = origins.add_expansion(&files, file, span, source, transform)?;

    assert_eq!(origins.len(), 2);
    assert!(!origins.is_empty());
    let stored_source = origins
        .get(source)
        .ok_or(SourceError::UnknownOrigin(source))?;
    assert_eq!(stored_source.location().file(), file);
    assert_eq!(stored_source.location().form(), form);
    assert_eq!(stored_source.location().span(), span);
    assert_eq!(stored_source.kind(), SourceOriginKind::Source);
    let stored_expansion = origins
        .get(expansion)
        .ok_or(SourceError::UnknownOrigin(expansion))?;
    assert_eq!(
        stored_expansion.kind(),
        SourceOriginKind::Expansion {
            parent: source,
            transform,
        }
    );
    assert_eq!(transform.kind(), SourceTransformKind::MacroExpansion);
    assert_eq!(transform.name(), transform_name);
    assert_eq!(
        origins.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        vec![source, expansion]
    );
    assert_eq!(
        origins.get(SourceOriginId::try_from_index(FormId::new(unit, 3), 0)?),
        None
    );
    Ok(())
}

#[test]
fn source_origins_reject_foreign_files_transforms_and_inconsistent_coordinates()
-> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(7);
    let form = FormId::new(unit, 0);
    let mut files = SourceFiles::new(unit);
    let file = files.add("one.lisp", "abc\ndef")?;
    let other_file = files.add("two.lisp", "abcdefgh")?;
    let foreign_unit = CompilationUnitId::new(8);
    let mut foreign_files = SourceFiles::new(foreign_unit);
    let foreign_file = foreign_files.add("foreign.lisp", "x")?;
    let mut origins = SourceOrigins::new(form);
    let source_span = files.span(file, 0, 3)?;
    let mismatched_span = files.span(file, 4, 7)?;
    let foreign_span = foreign_files.span(foreign_file, 0, 1)?;
    let source = origins.add_source(&files, file, source_span)?;

    assert_eq!(
        origins.add_source(&files, other_file, mismatched_span),
        Err(SourceError::InvalidCoordinates)
    );
    assert_eq!(
        origins.add_source(&files, foreign_file, foreign_span),
        Err(SourceError::ForeignFile(foreign_file))
    );
    let missing_parent = SourceOriginId::try_from_index(form, 9)?;
    let transform_name = SymbolId::try_from_index(unit, 1)?;
    let transform = SourceTransform::new(SourceTransformKind::CompilerTransform, transform_name);
    assert_eq!(
        origins.add_expansion(&files, file, source_span, missing_parent, transform),
        Err(SourceError::UnknownOrigin(missing_parent))
    );
    let foreign_name = SymbolId::try_from_index(foreign_unit, 1)?;
    let foreign_transform =
        SourceTransform::new(SourceTransformKind::CompilerMacroExpansion, foreign_name);
    assert_eq!(
        origins.add_expansion(&files, file, source_span, source, foreign_transform),
        Err(SourceError::ForeignTransform(foreign_name))
    );
    assert_eq!(
        SourceError::InvalidCoordinates.to_string(),
        "source error: InvalidCoordinates"
    );
    Ok(())
}
