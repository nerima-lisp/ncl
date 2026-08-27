use super::Value;

#[derive(Clone, Copy)]
pub enum FormatParameter {
    Missing,
    Number(i64),
    Character(char),
}

#[derive(Clone, Copy)]
pub struct FormatTermination {
    pub(super) colon_modifier: bool,
}

pub struct FormatDirective {
    pub(super) parameters: Vec<FormatParameter>,
    pub(super) directive: char,
    pub(super) colon_modifier: bool,
    pub(super) at_sign_modifier: bool,
}

pub struct FormatControlState<'a> {
    pub(super) characters: &'a [char],
    pub(super) arguments: &'a [Value],
    pub(super) output: &'a mut String,
    pub(super) argument_index: &'a mut usize,
    pub(super) character_index: &'a mut usize,
    pub(super) colon_iteration_last: bool,
}
