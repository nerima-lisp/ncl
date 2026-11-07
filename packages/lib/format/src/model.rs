//! Parsed FORMAT controls and directive metadata.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatControl {
    pub parts: Vec<ControlPart>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlPart {
    Literal(String),
    Directive(Directive),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directive {
    pub position: usize,
    pub parameters: Vec<Parameter>,
    pub colon: bool,
    pub at_sign: bool,
    pub kind: DirectiveKind,
    pub function_name: Option<String>,
    pub body: DirectiveBody,
    pub terminator: Option<Terminator>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectiveBody {
    None,
    Sequence(Box<FormatControl>),
    Clauses(Vec<ControlSegment>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlSegment {
    pub separator: Option<DirectiveModifiers>,
    pub control: FormatControl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectiveModifiers {
    pub parameters: Vec<Parameter>,
    pub colon: bool,
    pub at_sign: bool,
    pub position: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub colon: bool,
    pub at_sign: bool,
    pub position: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminatorKind {
    GreaterThan,
    RightBracket,
    RightBrace,
    RightParen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parameter {
    Integer(i64),
    Character(char),
    Relative,
    Remaining,
    Unsupplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectiveKind {
    A,
    S,
    W,
    C,
    R,
    D,
    B,
    O,
    X,
    F,
    E,
    G,
    Dollar,
    Percent,
    Ampersand,
    Bar,
    Tilde,
    Newline,
    Underscore,
    LogicalBlock,
    Indent,
    Slash,
    Tab,
    Justification,
    Star,
    Conditional,
    Iteration,
    Recursive,
    CaseConversion,
    Plural,
    UpArrow,
}

impl DirectiveKind {
    pub(crate) const fn from_character(character: char) -> Option<Self> {
        if matches!(character, 'A' | 'a') {
            Some(Self::A)
        } else if matches!(character, 'S' | 's') {
            Some(Self::S)
        } else if matches!(character, 'W' | 'w') {
            Some(Self::W)
        } else if matches!(character, 'C' | 'c') {
            Some(Self::C)
        } else if matches!(character, 'R' | 'r') {
            Some(Self::R)
        } else if matches!(character, 'D' | 'd') {
            Some(Self::D)
        } else if matches!(character, 'B' | 'b') {
            Some(Self::B)
        } else if matches!(character, 'O' | 'o') {
            Some(Self::O)
        } else if matches!(character, 'X' | 'x') {
            Some(Self::X)
        } else if matches!(character, 'F' | 'f') {
            Some(Self::F)
        } else if matches!(character, 'E' | 'e') {
            Some(Self::E)
        } else if matches!(character, 'G' | 'g') {
            Some(Self::G)
        } else if character == '$' {
            Some(Self::Dollar)
        } else if character == '%' {
            Some(Self::Percent)
        } else if character == '&' {
            Some(Self::Ampersand)
        } else if character == '|' {
            Some(Self::Bar)
        } else if character == '~' {
            Some(Self::Tilde)
        } else if character == '\n' {
            Some(Self::Newline)
        } else if character == '_' {
            Some(Self::Underscore)
        } else if character == '<' {
            Some(Self::Justification)
        } else if matches!(character, 'I' | 'i') {
            Some(Self::Indent)
        } else if character == '/' {
            Some(Self::Slash)
        } else if matches!(character, 'T' | 't') {
            Some(Self::Tab)
        } else if character == '*' {
            Some(Self::Star)
        } else if character == '[' {
            Some(Self::Conditional)
        } else if character == '{' {
            Some(Self::Iteration)
        } else if character == '?' {
            Some(Self::Recursive)
        } else if character == '(' {
            Some(Self::CaseConversion)
        } else if matches!(character, 'P' | 'p') {
            Some(Self::Plural)
        } else if character == '^' {
            Some(Self::UpArrow)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn character(self) -> char {
        match self {
            Self::A => 'A',
            Self::S => 'S',
            Self::W => 'W',
            Self::C => 'C',
            Self::R => 'R',
            Self::D => 'D',
            Self::B => 'B',
            Self::O => 'O',
            Self::X => 'X',
            Self::F => 'F',
            Self::E => 'E',
            Self::G => 'G',
            Self::Dollar => '$',
            Self::Percent => '%',
            Self::Ampersand => '&',
            Self::Bar => '|',
            Self::Tilde => '~',
            Self::Newline => '\n',
            Self::Underscore => '_',
            Self::LogicalBlock | Self::Justification => '<',
            Self::Indent => 'I',
            Self::Slash => '/',
            Self::Tab => 'T',
            Self::Star => '*',
            Self::Conditional => '[',
            Self::Iteration => '{',
            Self::Recursive => '?',
            Self::CaseConversion => '(',
            Self::Plural => 'P',
            Self::UpArrow => '^',
        }
    }
}

impl TerminatorKind {
    pub(crate) const fn from_character(character: char) -> Option<Self> {
        if character == '>' {
            Some(Self::GreaterThan)
        } else if character == ']' {
            Some(Self::RightBracket)
        } else if character == '}' {
            Some(Self::RightBrace)
        } else if character == ')' {
            Some(Self::RightParen)
        } else {
            None
        }
    }

    pub(crate) const fn character(self) -> char {
        match self {
            Self::GreaterThan => '>',
            Self::RightBracket => ']',
            Self::RightBrace => '}',
            Self::RightParen => ')',
        }
    }
}
