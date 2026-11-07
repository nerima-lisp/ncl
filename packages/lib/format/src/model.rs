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
        Some(match character {
            'A' | 'a' => Self::A,
            'S' | 's' => Self::S,
            'W' | 'w' => Self::W,
            'C' | 'c' => Self::C,
            'R' | 'r' => Self::R,
            'D' | 'd' => Self::D,
            'B' | 'b' => Self::B,
            'O' | 'o' => Self::O,
            'X' | 'x' => Self::X,
            'F' | 'f' => Self::F,
            'E' | 'e' => Self::E,
            'G' | 'g' => Self::G,
            '$' => Self::Dollar,
            '%' => Self::Percent,
            '&' => Self::Ampersand,
            '|' => Self::Bar,
            '~' => Self::Tilde,
            '\n' => Self::Newline,
            '_' => Self::Underscore,
            '<' => Self::Justification,
            'I' | 'i' => Self::Indent,
            '/' => Self::Slash,
            'T' | 't' => Self::Tab,
            '*' => Self::Star,
            '[' => Self::Conditional,
            '{' => Self::Iteration,
            '?' => Self::Recursive,
            '(' => Self::CaseConversion,
            'P' | 'p' => Self::Plural,
            '^' => Self::UpArrow,
            _ => return None,
        })
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
        match character {
            '>' => Some(Self::GreaterThan),
            ']' => Some(Self::RightBracket),
            '}' => Some(Self::RightBrace),
            ')' => Some(Self::RightParen),
            _ => None,
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
