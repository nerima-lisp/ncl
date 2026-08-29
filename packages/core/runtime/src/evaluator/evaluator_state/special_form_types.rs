use ncl_syntax::Form;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MacroLambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

pub struct SetfExpansion {
    pub temporaries: Vec<Form>,
    pub values: Vec<Form>,
    pub store: Form,
    pub store_form: Form,
    pub access_form: Form,
}
