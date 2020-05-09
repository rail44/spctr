use crate::token::*;
use cranelift::prelude::*;
use cranelift_codegen::binemit::NullTrapSink;
use cranelift_codegen::ir::{condcodes::FloatCC, types::*};
use cranelift_codegen::Context;
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};
use std::collections::HashMap;

pub fn compile(ast: &AST) -> *const u8 {
    let jit_builder = SimpleJITBuilder::new(cranelift_module::default_libcall_names());
    let mut module: Module<SimpleJITBackend> = Module::new(jit_builder);

    let mut scope_ctx = ScopeContext::new(module.make_context());

    let mut binds = HashMap::new();
    let mut translator = Translator::new(scope_ctx.get_builder(), &mut binds, &mut module);
    let ret = translator.translate(&ast);

    translator.builder.ins().return_(&[ret]);
    translator.builder.finalize();

    let id = module
        .declare_function("top", Linkage::Export, &scope_ctx.ctx.func.signature)
        .map_err(|e| e.to_string())
        .unwrap();
    module
        .define_function(id, &mut scope_ctx.ctx, &mut NullTrapSink {})
        .unwrap();
    module.clear_context(&mut scope_ctx.ctx);
    module.finalize_definitions();
    module.get_finalized_function(id)
}

struct ScopeContext {
    pub ctx: Context,
    builder_context: FunctionBuilderContext,
}

impl ScopeContext {
    fn new(mut ctx: Context) -> ScopeContext {
        let builder_context = FunctionBuilderContext::new();
        ctx.func.signature.returns.push(AbiParam::new(F64));

        ScopeContext {
            ctx,
            builder_context,
        }
    }

    fn get_builder(&mut self) -> FunctionBuilder {
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        builder.seal_block(block);
        builder
    }
}

struct Translator<'a> {
    binds: &'a mut HashMap<String, FuncId>,
    builder: FunctionBuilder<'a>,
    module: &'a mut Module<SimpleJITBackend>,
}

impl<'a> Translator<'a> {
    fn new(
        builder: FunctionBuilder<'a>,
        binds: &'a mut HashMap<String, FuncId>,
        module: &'a mut Module<SimpleJITBackend>,
    ) -> Translator<'a> {
        Translator {
            binds,
            builder,
            module,
        }
    }

    fn translate(&mut self, v: &Statement) -> Value {
        let mut b = Vec::new();

        for bind in v.definitions.iter() {
            let scope_ctx = ScopeContext::new(self.module.make_context());
            let id = self
                .module
                .declare_function(
                    &self.get_identifier(&bind.0),
                    Linkage::Local,
                    &scope_ctx.ctx.func.signature,
                )
                .map_err(|e| e.to_string())
                .unwrap();
            self.binds.insert(bind.0.clone(), id);
            b.push((scope_ctx, &bind.1, id));
        }

        for (mut scope_ctx, body, id) in b {
            let builder = scope_ctx.get_builder();
            let mut translator = Translator::new(builder, self.binds, &mut self.module);
            let ret = translator.translate_expression(&body);
            translator.builder.ins().return_(&[ret]);
            translator.builder.finalize();
            self.module
                .define_function(id, &mut scope_ctx.ctx, &mut NullTrapSink {})
                .unwrap();
            self.module.clear_context(&mut scope_ctx.ctx);
        }

        self.translate_expression(&v.body)
    }

    fn translate_expression(&mut self, v: &Expression) -> Value {
        match v {
            Expression::Comparison(a) => self.translate_comparison(a),
            Expression::If { cond, cons, alt } => {
                let cond_result = self.translate_expression(cond);
                let cons_block = self.builder.create_block();
                let alt_block = self.builder.create_block();

                let continuation = self.builder.create_block();

                self.builder.ins().brz(cond_result, alt_block, &[]);
                self.builder.ins().jump(cons_block, &[]);
                self.builder.switch_to_block(cons_block);
                self.builder.seal_block(cons_block);
                let cons_value = self.translate_expression(cons);
                self.builder.ins().jump(continuation, &[cons_value]);

                self.builder.switch_to_block(alt_block);
                self.builder.seal_block(alt_block);
                let alt_value = self.translate_expression(alt);
                self.builder.ins().jump(continuation, &[alt_value]);

                self.builder.append_block_param(continuation, F64);
                self.builder.switch_to_block(continuation);
                self.builder.seal_block(continuation);

                self.builder.block_params(continuation)[0]
            }
        }
    }

    fn translate_comparison(&mut self, v: &Comparison) -> Value {
        let mut lhs = self.translate_additive(&v.left);
        for right in &v.rights {
            match right {
                ComparisonRight::Equal(r) => {
                    let rhs = self.translate_additive(&r);
                    lhs = self.builder.ins().fcmp(FloatCC::Equal, lhs, rhs)
                }
                ComparisonRight::NotEqual(r) => {
                    let rhs = self.translate_additive(&r);
                    lhs = self.builder.ins().fcmp(FloatCC::NotEqual, lhs, rhs)
                }
            }
        }
        lhs
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
        lhs
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
        lhs
    }

    fn translate_primary(&mut self, v: &Primary) -> Value {
        match v {
            Primary::Number(v) => self.builder.ins().f64const(v.clone()),
            Primary::Identifier(name) => {
                let id = self.binds.get(name).unwrap();
                let func = self
                    .module
                    .declare_func_in_func(*id, &mut self.builder.func);
                let call = self.builder.ins().call(func, &[]);
                self.builder.inst_results(call)[0]
            }
            Primary::Block(s) => {
                let mut scope_ctx = ScopeContext::new(self.module.make_context());

                let mut binds = self.binds.clone();
                let mut translator =
                    Translator::new(scope_ctx.get_builder(), &mut binds, &mut self.module);
                let ret = translator.translate(&s);

                translator.builder.ins().return_(&[ret]);
                translator.builder.finalize();

                let id = self
                    .module
                    .declare_function(
                        &self.get_identifier("block"),
                        Linkage::Local,
                        &scope_ctx.ctx.func.signature,
                    )
                    .map_err(|e| e.to_string())
                    .unwrap();
                self.module
                    .define_function(id, &mut scope_ctx.ctx, &mut NullTrapSink {})
                    .unwrap();
                self.module.clear_context(&mut scope_ctx.ctx);
                let func = self.module.declare_func_in_func(id, &mut self.builder.func);
                let call = self.builder.ins().call(func, &[]);
                self.builder.inst_results(call)[0]
            }
        }
    }

    fn get_identifier(&self, name: &str) -> String {
        let mut i = 0;

        let mut identifier = format!("{}_{}", name, i);
        while self.module.get_name(&identifier).is_some() {
            identifier = format!("{}_{}", name, i);
            i += 1;
        }
        identifier
    }
}
