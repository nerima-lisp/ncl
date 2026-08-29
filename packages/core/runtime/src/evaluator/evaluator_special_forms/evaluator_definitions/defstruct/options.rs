use super::{
    Environment, Form, FormKind, HashSet, OrdinaryLambdaList, Runtime, RuntimeError, Span,
    StructureDefinition, atom_name, normalize_name, unqualified_name,
};

pub(super) struct DefstructOptions {
    pub(super) conc_name: String,
    pub(super) predicate_name: Option<String>,
    pub(super) copier_name: Option<String>,
    pub(super) constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)>,
    pub(super) included_structure: Option<(StructureDefinition, Vec<Form>)>,
}

impl Runtime {
    pub(super) fn parse_defstruct_options(
        structure_name: &str,
        option_forms: &[Form],
        environment: &Environment,
    ) -> Result<DefstructOptions, RuntimeError> {
        let mut conc_name = format!("{structure_name}-");
        let mut predicate_name = Some(format!("{structure_name}-P"));
        let mut copier_name = Some(format!("COPY-{structure_name}"));
        let mut constructor_options: Vec<(Option<String>, Option<OrdinaryLambdaList>)> = Vec::new();
        let mut seen_options = HashSet::new();
        let mut included_structure: Option<(StructureDefinition, Vec<Form>)> = None;
        for option_form in option_forms {
            let FormKind::List(option_items) = &option_form.kind else {
                return Err(Self::invalid(
                    "defstruct option must be a list",
                    option_form.span,
                ));
            };
            let Some(option_name) = option_items.first().and_then(atom_name) else {
                return Err(Self::invalid(
                    "defstruct option needs a name",
                    option_form.span,
                ));
            };
            let normalized_option = normalize_name(option_name);
            let option_name = normalized_option.trim_start_matches(':');
            Self::check_unique_defstruct_option(option_name, &mut seen_options, option_form.span)?;
            match option_name {
                "CONC-NAME" => {
                    conc_name = Self::defstruct_name_option(
                        option_form,
                        option_items,
                        format!("{structure_name}-"),
                        "defstruct :conc-name must name a symbol or NIL",
                    )?
                    .unwrap_or_default();
                }
                "PREDICATE" => {
                    predicate_name = Self::defstruct_name_option(
                        option_form,
                        option_items,
                        format!("{structure_name}-P"),
                        "defstruct :predicate must name a symbol or NIL",
                    )?;
                }
                "COPIER" => {
                    copier_name = Self::defstruct_name_option(
                        option_form,
                        option_items,
                        format!("COPY-{structure_name}"),
                        "defstruct :copier must name a symbol or NIL",
                    )?;
                }
                "INCLUDE" => {
                    if option_items.len() < 2 {
                        return Err(Self::invalid(
                            "defstruct :include needs a structure name",
                            option_form.span,
                        ));
                    }
                    let (raw_parent_name, _) = Self::variable_name_info(
                        &option_items[1],
                        "defstruct :include structure name must be a symbol",
                    )?;
                    let parent_name = unqualified_name(&raw_parent_name);
                    let Some(parent) = environment.lookup_structure(&parent_name) else {
                        return Err(Self::invalid(
                            "defstruct :include requires a previously defined structure",
                            option_form.span,
                        ));
                    };
                    included_structure = Some((parent, option_items[2..].to_vec()));
                }
                "CONSTRUCTOR" => {
                    let constructor = Self::defstruct_constructor_option(
                        option_form,
                        option_items,
                        format!("MAKE-{structure_name}"),
                    )?;
                    if (constructor.0.is_none() && !constructor_options.is_empty())
                        || constructor_options.iter().any(|(name, _)| name.is_none())
                    {
                        return Err(Self::invalid(
                            "defstruct :constructor NIL cannot be combined with another constructor",
                            option_form.span,
                        ));
                    }
                    constructor_options.push(constructor);
                }
                _ => {
                    return Err(Self::invalid(
                        "unsupported defstruct option",
                        option_items[0].span,
                    ));
                }
            }
        }
        Ok(DefstructOptions {
            conc_name,
            predicate_name,
            copier_name,
            constructor_options,
            included_structure,
        })
    }

    fn check_unique_defstruct_option(
        option_name: &str,
        seen_options: &mut HashSet<String>,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if option_name != "CONSTRUCTOR" && !seen_options.insert(option_name.to_string()) {
            return Err(Self::invalid("defstruct cannot repeat an option", span));
        }
        Ok(())
    }
}
