//! Parsing lambda lists into the frozen [`LambdaList`] type.
//!
//! The parser enforces the section order of CLHS 3.4.1 (`&whole`,
//! `&environment`, required, `&optional`, `&rest` or `&body`, `&key`,
//! `&allow-other-keys`, `&aux`) and rejects a repeated keyword or name.

use ncl_object::{ObjectRef, Word, classify_object};

use crate::error::FrontError;
use crate::expand::{FormExpander, LambdaListKind};
use crate::lambda_list::{AuxParam, KeyParam, LambdaList, OptionalParam, ParamName};
use crate::symbols::SymbolRef;

/// The section a parameter belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Section {
    /// Required parameters, which also follow `&whole` and `&environment`.
    Required,
    /// `&optional`.
    Optional,
    /// `&rest`.
    Rest,
    /// `&body`.
    Body,
    /// `&key`.
    Key,
    /// `&allow-other-keys`, after which no parameter may appear.
    AllowOtherKeys,
    /// `&aux`.
    Aux,
}

impl Section {
    /// The position of the section keyword in the lambda list.
    const fn rank(self) -> u8 {
        match self {
            Self::Required => 2,
            Self::Optional => 3,
            Self::Rest | Self::Body => 4,
            Self::Key => 5,
            Self::AllowOtherKeys => 6,
            Self::Aux => 7,
        }
    }

    /// The section named by a lambda-list keyword.
    fn from_keyword(keyword: &str) -> Option<Self> {
        Some(match keyword {
            "&WHOLE" | "&ENVIRONMENT" => Self::Required,
            "&OPTIONAL" => Self::Optional,
            "&REST" => Self::Rest,
            "&BODY" => Self::Body,
            "&KEY" => Self::Key,
            "&ALLOW-OTHER-KEYS" => Self::AllowOtherKeys,
            "&AUX" => Self::Aux,
            _ => return None,
        })
    }
}

impl FormExpander<'_> {
    /// Parse a lambda list.
    ///
    /// # Errors
    ///
    /// Returns [`FrontError::LambdaListOrder`] for a keyword out of order,
    /// [`FrontError::DuplicateLambdaListKeyword`] for a repeated keyword,
    /// [`FrontError::UnknownLambdaListKeyword`] for an unknown keyword or a
    /// macro-only keyword in an ordinary lambda list, and
    /// [`FrontError::DuplicateName`] for a repeated parameter name.
    pub fn expand_lambda_list(
        &mut self,
        form: Word,
        kind: LambdaListKind,
    ) -> Result<LambdaList, FrontError> {
        let elements = self.elements(form)?;
        let mut list = LambdaList::new();
        let mut section = Section::Required;
        let mut last_rank: Option<u8> = None;
        let mut seen: Vec<String> = Vec::new();
        let mut names: Vec<SymbolRef> = Vec::new();
        let mut index = 0;
        while index < elements.len() {
            let element = elements[index];
            index += 1;
            let Some(keyword) = self.lambda_list_keyword(element)? else {
                self.push_parameter(&mut list, kind, section, element, &mut names)?;
                continue;
            };
            let keyword_symbol = SymbolRef::interned("COMMON-LISP", &keyword);
            let Some(next) = Section::from_keyword(&keyword) else {
                return Err(FrontError::UnknownLambdaListKeyword {
                    keyword: keyword_symbol,
                });
            };
            let macro_only = matches!(keyword.as_str(), "&WHOLE" | "&ENVIRONMENT" | "&BODY");
            if macro_only && kind != LambdaListKind::Macro {
                return Err(FrontError::UnknownLambdaListKeyword {
                    keyword: keyword_symbol,
                });
            }
            if seen.iter().any(|entry| entry == &keyword) {
                return Err(FrontError::DuplicateLambdaListKeyword {
                    keyword: keyword_symbol,
                });
            }
            if keyword == "&ALLOW-OTHER-KEYS" && last_rank != Some(Section::Key.rank()) {
                return Err(FrontError::LambdaListOrder {
                    keyword: keyword_symbol,
                });
            }
            if last_rank.is_some_and(|previous| next.rank() <= previous) {
                return Err(FrontError::LambdaListOrder {
                    keyword: keyword_symbol,
                });
            }
            seen.push(keyword.clone());
            last_rank = Some(next.rank());
            section = next;
            if next == Section::AllowOtherKeys {
                list.allow_other_keys = true;
            }
            if matches!(keyword.as_str(), "&WHOLE" | "&ENVIRONMENT") {
                let Some(argument) = elements.get(index) else {
                    return Err(FrontError::WrongNumberOfForms {
                        operator: keyword_symbol,
                        expected: "a variable after the lambda list keyword",
                        found: 0,
                    });
                };
                index += 1;
                let name = self.symbol(*argument)?;
                if keyword == "&WHOLE" {
                    list.whole = Some(name);
                } else {
                    list.environment = Some(name);
                }
            }
        }
        Ok(list)
    }

    /// Append one parameter to the current section.
    fn push_parameter(
        &mut self,
        list: &mut LambdaList,
        kind: LambdaListKind,
        section: Section,
        element: Word,
        names: &mut Vec<SymbolRef>,
    ) -> Result<(), FrontError> {
        match section {
            Section::Required => {
                let name = self.parameter_name(kind, element)?;
                record_names(&name, names)?;
                list.required.push(name);
            }
            Section::Optional => {
                let parameter = self.optional_parameter(kind, element)?;
                record_names(&parameter.name, names)?;
                record_supplied(parameter.supplied_p.as_ref(), names)?;
                list.optional.push(parameter);
            }
            Section::Rest => {
                let name = self.parameter_name(kind, element)?;
                record_names(&name, names)?;
                list.rest = Some(name);
            }
            Section::Body => {
                let name = self.parameter_name(kind, element)?;
                record_names(&name, names)?;
                list.body = Some(name);
            }
            Section::Key => {
                let parameter = self.key_parameter(kind, element)?;
                record_names(&parameter.name, names)?;
                record_supplied(parameter.supplied_p.as_ref(), names)?;
                list.keys.push(parameter);
            }
            Section::AllowOtherKeys => {
                return Err(FrontError::LambdaListOrder {
                    keyword: SymbolRef::interned("COMMON-LISP", "&ALLOW-OTHER-KEYS"),
                });
            }
            Section::Aux => {
                let parameter = self.aux_parameter(kind, element)?;
                record_names(&parameter.name, names)?;
                list.aux.push(parameter);
            }
        }
        Ok(())
    }

    /// Read a lambda-list section keyword, if the word is one.
    fn lambda_list_keyword(&mut self, word: Word) -> Result<Option<String>, FrontError> {
        if !matches!(classify_object(self.ctx(), word), ObjectRef::Symbol(_)) {
            return Ok(None);
        }
        let name = self.symbol(word)?.name;
        Ok(name.starts_with('&').then_some(name))
    }

    /// Parse a required, rest, or body parameter.
    fn parameter_name(
        &mut self,
        kind: LambdaListKind,
        word: Word,
    ) -> Result<ParamName, FrontError> {
        if !word.is_cons() {
            return Ok(ParamName::Symbol(self.symbol(word)?));
        }
        if kind != LambdaListKind::Macro {
            return Err(FrontError::UnknownLambdaListKeyword {
                keyword: SymbolRef::interned("COMMON-LISP", "&DESTRUCTURING"),
            });
        }
        let nested = self.expand_lambda_list(word, LambdaListKind::Macro)?;
        Ok(ParamName::Pattern(Box::new(nested)))
    }

    /// Parse an `&optional` parameter.
    fn optional_parameter(
        &mut self,
        kind: LambdaListKind,
        word: Word,
    ) -> Result<OptionalParam, FrontError> {
        let parts = self.parameter_parts(word)?;
        let name = self.parameter_name(kind, parts[0])?;
        let default = parts.get(1).map(|form| self.expand(*form)).transpose()?;
        let supplied_p = parts
            .get(2)
            .map(|form| self.parameter_name(kind, *form))
            .transpose()?;
        Ok(OptionalParam {
            name,
            default,
            supplied_p,
        })
    }

    /// Parse an `&aux` parameter.
    fn aux_parameter(&mut self, kind: LambdaListKind, word: Word) -> Result<AuxParam, FrontError> {
        let parts = self.parameter_parts(word)?;
        let name = self.parameter_name(kind, parts[0])?;
        let default = parts.get(1).map(|form| self.expand(*form)).transpose()?;
        Ok(AuxParam { name, default })
    }

    /// Parse a `&key` parameter.
    fn key_parameter(&mut self, kind: LambdaListKind, word: Word) -> Result<KeyParam, FrontError> {
        let parts = self.parameter_parts(word)?;
        let (keyword, name) = self.key_names(kind, parts[0])?;
        let default = parts.get(1).map(|form| self.expand(*form)).transpose()?;
        let supplied_p = parts
            .get(2)
            .map(|form| self.parameter_name(kind, *form))
            .transpose()?;
        Ok(KeyParam {
            keyword,
            name,
            default,
            supplied_p,
        })
    }

    /// Resolve the keyword and variable of a `&key` parameter.
    fn key_names(
        &mut self,
        kind: LambdaListKind,
        word: Word,
    ) -> Result<(SymbolRef, ParamName), FrontError> {
        if !word.is_cons() {
            let name = self.symbol(word)?;
            return Ok((
                SymbolRef::keyword(name.name.clone()),
                ParamName::Symbol(name),
            ));
        }
        let parts = self.elements(word)?;
        let Some((keyword, rest)) = parts.split_first() else {
            return Err(FrontError::MalformedDeclaration {
                detail: "empty &key keyword specifier".to_owned(),
            });
        };
        let keyword = self.symbol(*keyword)?;
        let Some(name) = rest.first() else {
            return Err(FrontError::MalformedDeclaration {
                detail: "&key keyword specifier has no variable".to_owned(),
            });
        };
        Ok((keyword, self.parameter_name(kind, *name)?))
    }

    /// Split a parameter into its name and optional initializer parts.
    fn parameter_parts(&mut self, word: Word) -> Result<Vec<Word>, FrontError> {
        if word.is_cons() {
            return self.elements(word);
        }
        Ok(vec![word])
    }
}

/// Record the names bound by a parameter, rejecting duplicates.
fn record_names(name: &ParamName, names: &mut Vec<SymbolRef>) -> Result<(), FrontError> {
    match name {
        ParamName::Symbol(symbol) => {
            if names.iter().any(|existing| existing == symbol) {
                return Err(FrontError::DuplicateName {
                    name: symbol.clone(),
                });
            }
            names.push(symbol.clone());
        }
        ParamName::Pattern(pattern) => {
            for nested in &pattern.required {
                record_names(nested, names)?;
            }
        }
    }
    Ok(())
}

/// Record a supplied-p name, if present.
fn record_supplied(
    supplied: Option<&ParamName>,
    names: &mut Vec<SymbolRef>,
) -> Result<(), FrontError> {
    supplied.map_or(Ok(()), |supplied| record_names(supplied, names))
}
