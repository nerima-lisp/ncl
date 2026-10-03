#![allow(missing_docs)]

use ncl_object::{
    BuiltinArgs, LoadPort, MultipleValues, ObjectError, Package, Runtime, ThreadContext, Word,
    make_string,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

#[derive(Debug)]
struct EchoLoadPort;

impl LoadPort for EchoLoadPort {
    fn load(
        &self,
        _ctx: &mut ThreadContext,
        _runtime: &Runtime,
        args: &BuiltinArgs<'_>,
        values: &mut MultipleValues,
    ) -> Result<Word, ObjectError> {
        let path = args.required(0)?;
        let result = Word::fixnum(73);
        values.set(&[result, path]);
        Ok(result)
    }
}

#[test]
fn installed_load_port_receives_arguments_and_publishes_multiple_values() {
    let (runtime, mut context) = setup();
    runtime.set_load_port(Box::new(EchoLoadPort));
    let path = make_string(&mut context, &runtime, &['d', 'a', 't', 'a'])
        .unwrap_or_else(|error| panic!("path: {error:?}"));
    let argument_words = [path];
    let args = BuiltinArgs::new(&argument_words);
    let mut values = MultipleValues::new();

    assert_eq!(
        runtime.load_port(&mut context, &args, &mut values),
        Ok(Word::fixnum(73))
    );
    assert_eq!(values.as_slice(), &[Word::fixnum(73), path]);
}

#[test]
fn structure_class_resolves_registered_layout_to_the_rooted_class() {
    let (runtime, mut context) = setup();
    let package = Package::new(&mut context, &runtime, "EXECUTION-TEST")
        .unwrap_or_else(|error| panic!("package: {error:?}"));
    let (name, _) = package
        .intern(&mut context, &runtime, "NODE")
        .unwrap_or_else(|error| panic!("symbol: {error:?}"));
    let layout = runtime
        .register_structure_layout(1)
        .unwrap_or_else(|error| panic!("layout: {error:?}"));
    runtime
        .register_structure_class_with_parent(&context, layout, None, name)
        .unwrap_or_else(|error| panic!("class metadata: {error:?}"));
    runtime
        .define_class(&mut context, "EXECUTION-TEST::NODE", Word::TRUE)
        .unwrap_or_else(|error| panic!("class: {error:?}"));

    assert_eq!(
        runtime.structure_class(&mut context, layout),
        Some(Word::TRUE)
    );
    assert_eq!(runtime.structure_class(&mut context, 99.into()), None);
}
