use crate::translator::Translator;
use crate::vm::{Cmd, ForeignFunction, Value};
use std::rc::Rc;

pub fn get_module(translator: &mut Translator) -> Vec<Cmd> {
    let mut translator = translator.fork();
    let mut block = translator.block();

    block.add_bind("concat", concat);
    block.finalize()
}

fn concat(_: &mut Translator) -> Vec<Cmd> {
    vec![Cmd::ForeignFunction(ForeignFunction(Rc::new(
        move |_, mut args| {
            let mut target = (*args.pop().unwrap().into_list().unwrap()).clone();
            let mut dst = (*args.pop().unwrap().into_list().unwrap()).clone();
            target.append(&mut dst);
            Value::list(Rc::new(target))
        },
    )))]
}
