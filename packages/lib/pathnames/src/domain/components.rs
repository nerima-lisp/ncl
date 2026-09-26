//! Typed pathname components.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Host(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Device(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Component {
    Literal(String),
    Wild,
    WildInferiors,
}

impl Component {
    pub fn literal(value: impl Into<String>) -> Self {
        Self::Literal(value.into())
    }

    pub fn render(&self) -> &str {
        match self {
            Self::Literal(value) => value,
            Self::Wild => "*",
            Self::WildInferiors => "**",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectoryKind {
    Absolute,
    Relative,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DirectoryElement {
    Name(String),
    Wild,
    WildInferiors,
    Up,
    Back,
}

impl DirectoryElement {
    pub fn render(&self) -> &str {
        match self {
            Self::Name(value) => value,
            Self::Wild => "*",
            Self::WildInferiors => "**",
            Self::Up => "..",
            Self::Back => "back",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Directory {
    pub kind: DirectoryKind,
    pub elements: Vec<DirectoryElement>,
}

impl Directory {
    pub fn relative(elements: Vec<DirectoryElement>) -> Self {
        Self { kind: DirectoryKind::Relative, elements }
    }

    pub fn absolute(elements: Vec<DirectoryElement>) -> Self {
        Self { kind: DirectoryKind::Absolute, elements }
    }
}
