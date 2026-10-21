//! Parse and canonicalize logical pathname namestrings.

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ParsedLogicalNamestring {
    pub(super) host: String,
    pub(super) namestring: String,
    pub(super) version: LogicalVersion,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum LogicalVersion {
    Newest,
    Wild,
    Integer(Vec<u32>),
}

pub(super) fn parse(text: &str) -> Option<ParsedLogicalNamestring> {
    let normalized = text.to_ascii_uppercase();
    let (host, body) = normalized.split_once(':')?;
    if host.is_empty() || body.contains(':') {
        return None;
    }
    if !is_word(host) {
        return None;
    }

    let directory_end = body.rfind(';').map_or(0, |index| index + 1);
    let (directory, leaf) = body.split_at(directory_end);
    validate_directory(directory)?;
    let leaf_parts = leaf.split('.').collect::<Vec<_>>();
    if leaf_parts.len() > 3 {
        return None;
    }
    if leaf_parts.len() > 1 && leaf_parts.get(1)?.is_empty() {
        return None;
    }

    let mut canonical_body = body.to_owned();
    let version = if leaf_parts.len() == 3 {
        let version_text = *leaf_parts.get(2)?;
        match version_text {
            "*" => LogicalVersion::Wild,
            "NEWEST" => LogicalVersion::Newest,
            numeric => {
                let limbs = parse_positive_integer(numeric)?;
                let normalized = decimal_from_limbs(&limbs)?;
                let canonical_version_start = directory_end
                    .checked_add(leaf_parts.first()?.len())?
                    .checked_add(1)?
                    .checked_add(leaf_parts.get(1)?.len())?
                    .checked_add(1)?;
                canonical_body.replace_range(canonical_version_start.., &normalized);
                LogicalVersion::Integer(limbs)
            }
        }
    } else {
        LogicalVersion::Newest
    };

    for component in leaf_parts.iter().take(2) {
        if !component.is_empty() {
            validate_wildcard_word(component)?;
        }
    }

    let namestring = format!("{host}:{canonical_body}");
    Some(ParsedLogicalNamestring {
        host: host.to_owned(),
        namestring,
        version,
    })
}

fn validate_directory(directory: &str) -> Option<()> {
    if directory.is_empty() {
        return Some(());
    }
    let components = directory.strip_suffix(';')?;
    let mut components = components.split(';');
    let first = components.next()?;
    if !first.is_empty() {
        validate_directory_component(first)?;
    }
    for component in components {
        validate_directory_component(component)?;
    }
    Some(())
}

fn validate_directory_component(component: &str) -> Option<()> {
    if component == "**" {
        Some(())
    } else {
        validate_wildcard_word(component)
    }
}

fn validate_wildcard_word(component: &str) -> Option<()> {
    if component.is_empty() {
        return None;
    }
    let mut previous_wildcard = false;
    for character in component.chars() {
        let wildcard = character == '*';
        if wildcard && previous_wildcard {
            return None;
        }
        if !wildcard
            && !character.is_ascii_uppercase()
            && !character.is_ascii_digit()
            && character != '-'
        {
            return None;
        }
        previous_wildcard = wildcard;
    }
    Some(())
}

fn is_word(component: &str) -> bool {
    !component.is_empty()
        && component.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '-'
        })
}

fn parse_positive_integer(value: &str) -> Option<Vec<u32>> {
    if value.is_empty() {
        return None;
    }
    let mut limbs = vec![0_u32];
    for character in value.chars() {
        let digit = character
            .to_digit(10)
            .filter(|_| character.is_ascii_digit())?;
        let mut carry = digit;
        for limb in &mut limbs {
            let next = u64::from(*limb) * 10 + u64::from(carry);
            *limb = u32::try_from(next & u64::from(u32::MAX)).ok()?;
            carry = u32::try_from(next >> 32).ok()?;
        }
        if carry != 0 {
            limbs.push(carry);
        }
    }
    while limbs.last() == Some(&0) && limbs.len() > 1 {
        limbs.pop();
    }
    if limbs.len() == 1 && limbs.first() == Some(&0) {
        return None;
    }
    Some(limbs)
}

fn decimal_from_limbs(limbs: &[u32]) -> Option<String> {
    let mut remaining = limbs.to_vec();
    let mut digits = Vec::new();
    while remaining.iter().any(|limb| *limb != 0) {
        let mut carry = 0_u64;
        for limb in remaining.iter_mut().rev() {
            let current = (carry << 32) | u64::from(*limb);
            *limb = u32::try_from(current / 10).ok()?;
            carry = current % 10;
        }
        let digit = u8::try_from(carry).ok()?;
        digits.push(char::from(b'0' + digit));
    }
    Some(digits.iter().rev().collect())
}
