//! POSIX parsing policy: `/` means an absolute directory, `..` means `up`,
//! `*` and `**` are wild directory/name components, and `~` is literal text.
//! No home-directory expansion, filesystem access, or other environment lookup occurs.

use super::{
    Component, Device, Directory, DirectoryElement, DirectoryKind, Host, Pathname, PathnameError,
    Version,
};

pub fn parse_namestring(input: &str) -> Result<Pathname, PathnameError> {
    if input.is_empty() {
        return Err(PathnameError::EmptyNamestring);
    }
    let (host, rest) = split_prefix(input, ':').map_or((None, input), |(value, rest)| {
        (Some(Host(value.to_string())), rest)
    });
    let (device, rest) = split_prefix(rest, ':').map_or((None, rest), |(value, rest)| {
        (Some(Device(value.to_string())), rest)
    });
    let absolute = rest.starts_with('/');
    let trimmed = rest.trim_start_matches('/');
    let mut parts = trimmed.split('/').collect::<Vec<_>>();
    let file = if rest.ends_with('/') {
        None
    } else {
        parts.pop().filter(|part| !part.is_empty())
    };
    let elements = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(parse_directory_element)
        .collect::<Result<Vec<_>, _>>()?;
    let (name, type_, version) = file
        .map(parse_file)
        .transpose()?
        .unwrap_or((None, None, None));
    Ok(Pathname {
        host,
        device,
        directory: Some(Directory {
            kind: if absolute {
                DirectoryKind::Absolute
            } else {
                DirectoryKind::Relative
            },
            elements,
        }),
        name,
        type_,
        version,
    })
}

fn split_prefix(input: &str, separator: char) -> Option<(&str, &str)> {
    let (prefix, rest) = input.split_once(separator)?;
    (!prefix.is_empty()).then_some((prefix, rest))
}

fn parse_directory_element(value: &str) -> Result<DirectoryElement, PathnameError> {
    match value {
        "*" => Ok(DirectoryElement::Wild),
        "**" => Ok(DirectoryElement::WildInferiors),
        ".." => Ok(DirectoryElement::Up),
        "back" => Ok(DirectoryElement::Back),
        value if value.contains(';') => Err(PathnameError::InvalidComponent(value.to_string())),
        value => Ok(DirectoryElement::Name(value.to_string())),
    }
}

fn parse_file(
    value: &str,
) -> Result<(Option<Component>, Option<Component>, Option<Version>), PathnameError> {
    let (stem, version) = match value.split_once(';') {
        Some((stem, version)) if !version.is_empty() => (stem, Some(Version(version.to_string()))),
        Some(_) => return Err(PathnameError::InvalidVersion(value.to_string())),
        None => (value, None),
    };
    let (name, type_) = match stem.rsplit_once('.') {
        Some((name, type_)) if !name.is_empty() && !type_.is_empty() => (name, Some(type_)),
        _ => (stem, None),
    };
    let parse = |component: &str| match component {
        "*" => Component::Wild,
        "**" => Component::WildInferiors,
        value => Component::Literal(value.to_string()),
    };
    Ok((Some(parse(name)), type_.map(parse), version))
}

pub fn make_pathname(defaults: &Pathname, supplied: &Pathname) -> Pathname {
    Pathname {
        host: supplied.host.clone().or_else(|| defaults.host.clone()),
        device: supplied.device.clone().or_else(|| defaults.device.clone()),
        directory: supplied
            .directory
            .clone()
            .or_else(|| defaults.directory.clone()),
        name: supplied.name.clone().or_else(|| defaults.name.clone()),
        type_: supplied.type_.clone().or_else(|| defaults.type_.clone()),
        version: supplied
            .version
            .clone()
            .or_else(|| defaults.version.clone()),
    }
}

pub fn merge_pathnames(primary: &Pathname, defaults: &Pathname) -> Pathname {
    make_pathname(defaults, primary)
}
