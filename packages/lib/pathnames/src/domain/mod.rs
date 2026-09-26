//! Pure typed pathname domain.
//!
//! Parsing is POSIX-shaped and deliberately does not expand `~`, consult the
//! filesystem, or read environment variables. It treats a leading `/` as an
//! absolute directory, preserves ordinary text literally, and recognizes
//! `*`, `**`, `..`, and `back` as typed directory elements.

mod components;
mod error;
mod matching;
mod parse;
mod pathname;
mod translate;

pub use components::{
    Component, Device, Directory, DirectoryElement, DirectoryKind, Host, Name, Type, Version,
};
pub use error::PathnameError;
pub use matching::{pathname_match_p, wild_pathname_p};
pub use parse::{make_pathname, merge_pathnames, parse_namestring};
pub use pathname::Pathname;
pub use translate::translate_pathname;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_posix_components_without_tilde_expansion() {
        let pathname = parse_namestring("~/src/*.rs;3").expect("valid pathname");
        assert_eq!(
            pathname.directory.as_ref().map(|value| value.kind),
            Some(DirectoryKind::Relative)
        );
        assert_eq!(
            pathname.directory.as_ref().map(|value| &value.elements),
            Some(&vec![
                DirectoryElement::Name("~".into()),
                DirectoryElement::Name("src".into())
            ])
        );
        assert_eq!(pathname.name, Some(Component::Wild));
        assert_eq!(pathname.type_, Some(Component::Literal("rs".into())));
        assert_eq!(pathname.version, Some(Version("3".into())));
        assert_eq!(pathname.namestring(), "~/src/*.rs;3");
    }

    #[test]
    fn renders_absolute_directories_and_special_elements() {
        let pathname = parse_namestring("/a/../**/file").expect("valid pathname");
        assert_eq!(pathname.namestring(), "/a/../**/file");
        assert!(!wild_pathname_p(
            &parse_namestring("/a/file").expect("valid pathname")
        ));
        assert!(wild_pathname_p(&pathname));
    }

    #[test]
    fn matches_wild_inferiors() {
        let value = parse_namestring("/a/b/c.txt").expect("valid pathname");
        let pattern = parse_namestring("/**/c.txt").expect("valid pathname");
        assert!(pathname_match_p(&value, &pattern));
    }

    #[test]
    fn merges_and_translates_pure_values() {
        let defaults = parse_namestring("/tmp/default.txt").expect("valid pathname");
        let supplied = parse_namestring("result.txt").expect("valid pathname");
        let merged = merge_pathnames(&supplied, &defaults);
        assert_eq!(merged.namestring(), "result.txt");
        let from = parse_namestring("/tmp/*.txt").expect("valid pathname");
        let to = parse_namestring("/out/*.bak").expect("valid pathname");
        let source = parse_namestring("/tmp/report.txt").expect("valid pathname");
        assert_eq!(
            translate_pathname(&source, &from, &to)
                .expect("matching pathname")
                .namestring(),
            "/out/report.bak"
        );
    }
}
