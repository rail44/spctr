use crate::token::*;
use cranelift::prelude::*;
use cranelift_module::{DataContext, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};
use cranelift_codegen::ir::types::*;
use cranelift_codegen::binemit::NullTrapSink;
use std::collections::HashMap;

pub fn compile(ast: &AST) -> *const u8 {
    let jit_builder = SimpleJITBuilder::new(cranelift_module::default_libcall_names());
    let mut module: Module<SimpleJITBackend> = Module::new(jit_builder);
    let mut ctx = module.make_context();
    ctx.func.signature.returns.push(AbiParam::new(F64));
    let data_ctx = DataContext::new();
    let mut builder_context = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let mut translator = Translator::new(&mut builder);
    translator.translate(&ast);

    let id = module
        .declare_function("top", Linkage::Export, &ctx.func.signature)
        .map_err(|e| e.to_string()).unwrap();
    module.define_function(id, &mut ctx, &mut NullTrapSink {}).unwrap();
    module.clear_context(&mut ctx);
    module.finalize_definitions();
    module.get_finalized_function(id)
}

struct Translator<'a> {
    binds: HashMap<String, Variable>,
    builder: &'a mut FunctionBuilder<'a>
}

impl<'a> Translator<'a> {
    fn new(builder: &'a mut FunctionBuilder<'a>) -> Translator<'a> {
        Translator {
            binds: HashMap::new(),
            builder,
        }
    }

    fn translate(&mut self, v: &AST) {
        let v = self.translate_statement(v);

        self.builder.ins().return_(&[v]);
        self.builder.finalize();
    }

    fn translate_statement(&mut self, v: &Statement) -> Value {
        for (i, bind) in v.definitions.iter().enumerate() {
            let variable = Variable::new(i);
            let value = self.translate_additive(&bind.1);
            self.builder.declare_var(variable, F64);
            self.builder.def_var(variable, value);
            self.binds.insert(bind.0.clone(), variable);
        }
        self.translate_additive(&v.body)
    }

    fn translate_additive(&mut self, v: &Additive) -> Value {
        let mut lhs = self.translate_multitive(&v.left);
        for right in &v.rights {
            match right {
                AdditiveRight::Add(r) => {
                    let rhs = self.translate_multitive(&r);
                    lhs = self.builder.ins().fadd(lhs, rhs);
                }
                AdditiveRight::Sub(r) => {
                    let rhs = self.translate_multitive(&r);
                    lhs = self.builder.ins().fsub(lhs, rhs);
                }
            }
        }
        return lhs;
    }

    fn translate_multitive(&mut self, v: &Multitive) -> Value {
        let mut lhs = self.translate_primary(&v.left);
        for right in &v.rights {
            match right {
                MultitiveRight::Mul(r) => {
                    let rhs = self.translate_primary(&r);
                    lhs = self.builder.ins().fmul(lhs, rhs);
                }
                MultitiveRight::Div(r) => {
                    let rhs = self.translate_primary(&r);
                    lhs = self.builder.ins().fdiv(lhs, rhs);
                }
            }
        }
        return lhs;
    }

    fn translate_primary(&mut self, v: &Primary) -> Value {
        match v {
            Primary::Number(v) => {
                self.builder.ins().f64const(v.clone())
            }
            Primary::Identifier(name) => {
                self.builder.use_var(self.binds.get(name).unwrap().clone())
            }
            _ => unimplemented!(),
        }
    }
}
