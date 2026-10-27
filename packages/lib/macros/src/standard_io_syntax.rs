use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

use super::{PlaceRegistry, form};

enum BindingValue {
    ClUserPackage,
    True,
    Ten,
    Upcase,
    Nil,
    StandardPprintDispatch,
    SingleFloat,
    StandardReadtable,
}

const STANDARD_BINDINGS: &[(&str, BindingValue)] = &[
    ("*PACKAGE*", BindingValue::ClUserPackage),
    ("*PRINT-ARRAY*", BindingValue::True),
    ("*PRINT-BASE*", BindingValue::Ten),
    ("*PRINT-CASE*", BindingValue::Upcase),
    ("*PRINT-CIRCLE*", BindingValue::Nil),
    ("*PRINT-ESCAPE*", BindingValue::True),
    ("*PRINT-GENSYM*", BindingValue::True),
    ("*PRINT-LENGTH*", BindingValue::Nil),
    ("*PRINT-LEVEL*", BindingValue::Nil),
    ("*PRINT-LINES*", BindingValue::Nil),
    ("*PRINT-MISER-WIDTH*", BindingValue::Nil),
    (
        "*PRINT-PPRINT-DISPATCH*",
        BindingValue::StandardPprintDispatch,
    ),
    ("*PRINT-PRETTY*", BindingValue::Nil),
    ("*PRINT-RADIX*", BindingValue::Nil),
    ("*PRINT-READABLY*", BindingValue::True),
    ("*PRINT-RIGHT-MARGIN*", BindingValue::Nil),
    ("*READ-BASE*", BindingValue::Ten),
    ("*READ-DEFAULT-FLOAT-FORMAT*", BindingValue::SingleFloat),
    ("*READ-EVAL*", BindingValue::True),
    ("*READ-SUPPRESS*", BindingValue::Nil),
    ("*READTABLE*", BindingValue::StandardReadtable),
    ("NCL-EXT::*PRINT-CIRCLE-NOT-SHARED*", BindingValue::Nil),
    ("NCL-EXT::*PRINT-VECTOR-LENGTH*", BindingValue::Nil),
];

pub fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _places: PlaceRegistry,
    args: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, args, |ctx, rooted_args| {
        let mut roots = Vec::<Box<Word>>::new();
        let mut root_tokens = Vec::new();
        let mut binding_roots = Vec::with_capacity(23);

        macro_rules! root_word {
            ($value:expr) => {{
                let mut value = Box::new($value);
                root_tokens.push(ncl_object::push_root(ctx, value.as_mut()));
                roots.push(value);
                roots.len() - 1
            }};
        }
        macro_rules! rooted_word {
            ($index:expr) => {
                **roots.get($index).ok_or(ObjectError::Layout)?
            };
        }
        macro_rules! binding {
            ($name:expr, $value:expr) => {{
                let variable = root_word!(form::symbol(ctx, runtime, $name)?);
                let value = root_word!($value);
                let binding =
                    form::list(ctx, runtime, &[rooted_word!(variable), rooted_word!(value)])?;
                binding_roots.push(root_word!(binding));
            }};
        }

        let result = (|| {
            let truth = root_word!(form::symbol(ctx, runtime, "T")?);
            let upcase = root_word!(form::symbol(ctx, runtime, "KEYWORD::UPCASE")?);
            let single_float = root_word!(form::symbol(ctx, runtime, "SINGLE-FLOAT")?);

            let find_package = root_word!(form::symbol(ctx, runtime, "FIND-PACKAGE")?);
            let cl_user_name = root_word!(ncl_object::make_string(
                ctx,
                runtime,
                &"CL-USER".chars().collect::<Vec<_>>(),
            )?);
            let cl_user_package = root_word!(form::list(
                ctx,
                runtime,
                &[rooted_word!(find_package), rooted_word!(cl_user_name),],
            )?);

            let copy_pprint_dispatch =
                root_word!(form::symbol(ctx, runtime, "COPY-PPRINT-DISPATCH")?);
            let standard_pprint_dispatch = root_word!(form::list(
                ctx,
                runtime,
                &[rooted_word!(copy_pprint_dispatch), Word::NIL],
            )?);

            let copy_readtable = root_word!(form::symbol(ctx, runtime, "COPY-READTABLE")?);
            let standard_readtable = root_word!(form::list(
                ctx,
                runtime,
                &[rooted_word!(copy_readtable), Word::NIL],
            )?);
            let quote = root_word!(form::symbol(ctx, runtime, "QUOTE")?);
            let standard_float_format = root_word!(form::list(
                ctx,
                runtime,
                &[rooted_word!(quote), rooted_word!(single_float)],
            )?);

            for (name, value_kind) in STANDARD_BINDINGS {
                let value = match value_kind {
                    BindingValue::ClUserPackage => rooted_word!(cl_user_package),
                    BindingValue::True => rooted_word!(truth),
                    BindingValue::Ten => Word::fixnum(10),
                    BindingValue::Upcase => rooted_word!(upcase),
                    BindingValue::Nil => Word::NIL,
                    BindingValue::StandardPprintDispatch => {
                        rooted_word!(standard_pprint_dispatch)
                    }
                    BindingValue::SingleFloat => rooted_word!(standard_float_format),
                    BindingValue::StandardReadtable => rooted_word!(standard_readtable),
                };
                binding!(*name, value);
            }

            let mut binding_forms = Vec::with_capacity(binding_roots.len());
            for index in &binding_roots {
                binding_forms.push(rooted_word!(*index));
            }
            let bindings = root_word!(form::list(ctx, runtime, &binding_forms)?);
            let let_symbol = root_word!(form::symbol(ctx, runtime, "LET")?);
            let mut expansion = Vec::with_capacity(rooted_args.len() + 2);
            expansion.push(rooted_word!(let_symbol));
            expansion.push(rooted_word!(bindings));
            expansion.extend(rooted_args.iter().map(|argument| **argument));
            form::list(ctx, runtime, &expansion)
        })();

        for token in root_tokens.into_iter().rev() {
            ncl_object::pop_root(ctx, token);
        }
        result
    })
}
