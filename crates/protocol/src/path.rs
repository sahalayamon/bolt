//! Filename and relative-path safety. Used by both sides: the sender
//! sanitises names it produces, the receiver *rejects* anything unsafe.

use thiserror::Error;

pub const MAX_COMPONENT_BYTES: usize = 255;
pub const MAX_PATH_BYTES: usize = 1024;

#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum PathError {
    #[error("empty path or component")]
    Empty,
    #[error("path or component too long")]
    TooLong,
    #[error("absolute or drive-qualified path")]
    Absolute,
    #[error("'.' or '..' component")]
    Traversal,
    #[error("forbidden character")]
    ForbiddenChar,
    #[error("reserved device name")]
    Reserved,
    #[error("trailing dot or space")]
    TrailingDotOrSpace,
}

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn is_forbidden(c: char) -> bool {
    c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/')
}

fn is_reserved(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or("");
    RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem))
}

pub fn validate_component(c: &str) -> Result<(), PathError> {
    if c.is_empty() {
        return Err(PathError::Empty);
    }
    if c.len() > MAX_COMPONENT_BYTES {
        return Err(PathError::TooLong);
    }
    if c == "." || c == ".." {
        return Err(PathError::Traversal);
    }
    if c.chars().any(is_forbidden) {
        return Err(PathError::ForbiddenChar);
    }
    if c.ends_with('.') || c.ends_with(' ') {
        return Err(PathError::TrailingDotOrSpace);
    }
    if is_reserved(c) {
        return Err(PathError::Reserved);
    }
    Ok(())
}

/// Validate a `/`-separated relative path received from a peer.
pub fn validate_rel_path(p: &str) -> Result<(), PathError> {
    if p.is_empty() {
        return Err(PathError::Empty);
    }
    if p.len() > MAX_PATH_BYTES {
        return Err(PathError::TooLong);
    }
    if p.starts_with('/') || p.starts_with('\\') {
        return Err(PathError::Absolute);
    }
    p.split('/').try_for_each(validate_component)
}

/// Turn an arbitrary local file name into one that passes [`validate_component`].
pub fn sanitize_component(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| if is_forbidden(c) { '_' } else { c })
        .collect();
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }
    if s.is_empty() || s == "." || s == ".." {
        s = "_".into();
    }
    if is_reserved(&s) {
        s.insert(0, '_');
    }
    while s.len() > MAX_COMPONENT_BYTES {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dangerous() {
        for bad in [
            "",
            "..",
            "a/..",
            "a//b",
            "/etc/passwd",
            "\\x",
            "C:\\x",
            "c:x",
            "a\\b",
            "CON",
            "nul.txt",
            "a.",
            "a ",
            "a\0b",
            "a/b/./c",
            "x\n",
        ] {
            assert!(
                validate_rel_path(bad).is_err(),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn accepts_unicode_and_spaces() {
        for ok in [
            "photo 1.jpg",
            "日本語/ファイル.txt",
            "emoji🙂.png",
            ".hidden",
            "a/b/c.tar.gz",
        ] {
            validate_rel_path(ok).unwrap_or_else(|e| panic!("{ok:?}: {e}"));
        }
    }

    #[test]
    fn sanitized_names_always_validate() {
        for raw in [
            "a:b",
            "con",
            "  ",
            "..",
            "x.",
            "q?*",
            "ok.txt",
            "a\\b",
            &"z".repeat(400),
            "nul.txt",
        ] {
            validate_component(&sanitize_component(raw)).unwrap_or_else(|e| panic!("{raw:?}: {e}"));
        }
    }
}
