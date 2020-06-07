use crate::translator::Translator;
use crate::vm::{Cmd, Value};
use std::rc::Rc;

pub fn get_module(translator: &mut Translator) -> Vec<Cmd> {
    let mut block = translator.block();

    block.add_bind("concat", |translator| translator.translate_foreign(concat));
    block.finalize()
}

fn concat(mut args: Vec<Value>) -> Value {
    let mut target = (*args.pop().unwrap().into_list().unwrap()).clone();
    let mut dst = (*args.pop().unwrap().into_list().unwrap()).clone();
    target.append(&mut dst);
    Value::list(Rc::new(target))
}
