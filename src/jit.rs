use crate::token::*;
use cranelift::prelude::*;
use cranelift_codegen::binemit::NullTrapSink;
use cranelift_codegen::ir::{condcodes::IntCC, types::*};
use cranelift_codegen::Context;
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};
use std::collections::HashMap;

struct SpctrValue([Value; 64]);

impl SpctrValue {
    fn new() -> SpctrValue {
        SpctrValue([Value::new(0); 64])
    }

    fn from_value(v: Value) -> SpctrValue {
        let mut spctr_v = SpctrValue::new();
        spctr_v.0[0] = v;
        spctr_v
    }

    fn from_slice(slice: &[Value]) -> SpctrValue {
        let mut v = SpctrValue::new();
        for (i, b) in slice.iter().enumerate() {
            v.0[i] = b.clone();
        }
        v
    }

    fn as_slice(&self) -> &[Value] {
        &self.0
    }

    fn into_array(self) -> [Value; 64] {
        self.0
    }

    fn get_first(self) -> Value {
        self.0[0]
    }
}

pub fn compile(ast: &AST) -> *const u8 {
    let jit_builder = SimpleJITBuilder::new(cranelift_module::default_libcall_names());
    let mut module: Module<SimpleJITBackend> = Module::new(jit_builder);

    let mut scope_ctx = ScopeContext::new(module.make_context());

    let mut binds = HashMap::new();
    let builder = scope_ctx.get_builder();
    let mut translator = Translator::new(builder, &mut binds, &mut module);
    let ret = translator.translate(&ast);

    translator.builder.ins().return_(ret.as_slice());
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
        for _ in 0..64 {
            ctx.func.signature.returns.push(AbiParam::new(I64));
        }

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
        builder.ins().iconst(I64, 0);
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

    fn translate(&mut self, v: &Statement) -> SpctrValue {
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
            translator.builder.ins().return_(ret.as_slice());

            translator.builder.finalize();
            self.module
                .define_function(id, &mut scope_ctx.ctx, &mut NullTrapSink {})
                .unwrap();
            self.module.clear_context(&mut scope_ctx.ctx);
        }

        self.translate_expression(&v.body)
    }

    fn translate_expression(&mut self, v: &Expression) -> SpctrValue {
        match v {
            Expression::Comparison(a) => self.translate_comparison(a),
            Expression::If { cond, cons, alt } => {
                let cond_result = self.translate_expression(cond);
                let cons_block = self.builder.create_block();
                let alt_block = self.builder.create_block();

                let continuation = self.builder.create_block();

                self.builder.ins().brz(cond_result.get_first(), alt_block, &[]);
                self.builder.ins().jump(cons_block, &[]);
                self.builder.switch_to_block(cons_block);
                self.builder.seal_block(cons_block);
                let cons_value = self.translate_expression(cons);
                self.builder.ins().jump(continuation, cons_value.as_slice());

                self.builder.switch_to_block(alt_block);
                self.builder.seal_block(alt_block);
                let alt_value = self.translate_expression(alt);
                self.builder.ins().jump(continuation, alt_value.as_slice());


                for _ in 0..64 {
                    self.builder.append_block_param(continuation, I64);
                }

                self.builder.switch_to_block(continuation);
                self.builder.seal_block(continuation);

                SpctrValue::from_slice(self.builder.block_params(continuation))
            }
        }
    }

    fn translate_comparison(&mut self, v: &Comparison) -> SpctrValue {
        let mut lhs = self.translate_additive(&v.left);
        for right in &v.rights {
            match right {
                ComparisonRight::Equal(r) => {
                    let rhs = self.translate_additive(&r);
                    let l_array = lhs.into_array();
                    let r_array = rhs.into_array();
                    let mut result = self.builder.ins().bconst(B1, true);
                    for (l, r) in l_array.iter().zip(r_array.iter()) {
                        let b = self.builder.ins().icmp(IntCC::Equal, *l, *r);
                        result = self.builder.ins().band(result, b);
                    }
                    lhs = SpctrValue::from_value(result);
                }
                ComparisonRight::NotEqual(r) => {
                    let rhs = self.translate_additive(&r);
                    let l_array = lhs.into_array();
                    let r_array = rhs.into_array();
                    let mut result = self.builder.ins().bconst(B1, true);
                    for (l, r) in l_array.iter().zip(r_array.iter()) {
                        let b = self.builder.ins().icmp(IntCC::NotEqual, *l, *r);
                        result = self.builder.ins().band(result, b);
                    }
                    lhs = SpctrValue::from_value(result);
                }
            }
        }
        lhs
    }

    fn translate_additive(&mut self, v: &Additive) -> SpctrValue {
        let mut lhs = self.translate_multitive(&v.left);
        for right in &v.rights {
            match right {
                AdditiveRight::Add(r) => {
                    let rhs = self.translate_multitive(&r);
                    let r = self.builder.ins().bitcast(F64, rhs.get_first());
                    let l = self.builder.ins().bitcast(F64, lhs.get_first());
                    let v = self.builder.ins().fadd(r, l);
                    lhs = SpctrValue::from_value(self.builder.ins().bitcast(I64, v));
                }
                AdditiveRight::Sub(r) => {
                    let rhs = self.translate_multitive(&r);
                    lhs = SpctrValue::from_value(self.builder.ins().fsub(lhs.get_first(), rhs.get_first()))
                }
            }
        }
        lhs
    }

    fn translate_multitive(&mut self, v: &Multitive) -> SpctrValue {
        let mut lhs = self.translate_primary(&v.left);
        for right in &v.rights {
            match right {
                MultitiveRight::Mul(r) => {
                    let rhs = self.translate_primary(&r);
                    lhs = SpctrValue::from_value(self.builder.ins().fmul(lhs.get_first(), rhs.get_first()))
                }
                MultitiveRight::Div(r) => {
                    let rhs = self.translate_primary(&r);
                    lhs = SpctrValue::from_value(self.builder.ins().fdiv(lhs.get_first(), rhs.get_first()))
                }
            }
        }
        lhs
    }

    fn translate_primary(&mut self, v: &Primary) -> SpctrValue {
        match v {
            Primary::Number(v) => {
                let v = self.builder.ins().f64const(v.clone());
                SpctrValue::from_value(self.builder.ins().bitcast(I64, v))
            }
            Primary::String(s) => {
                let mut bytes = s.clone().into_bytes();
                if bytes.len() % 8 > 0 {
                    let i = 8 - (bytes.len() % 8);
                    for _ in 0..i {
                        bytes.push(0);
                    }
                }
                let (_, eight_bytes, _) = unsafe { bytes.align_to::<u64>() };

                let values: Vec<Value> = eight_bytes.iter().map(|b| {
                    self.builder.ins().iconst(I64, *b as i64)
                }).collect();
                SpctrValue::from_slice(&values)
            }
            Primary::Identifier(name) => {
                let id = self.binds.get(name).unwrap();
                let func = self
                    .module
                    .declare_func_in_func(*id, &mut self.builder.func);
                let call = self.builder.ins().call(func, &[]);

                SpctrValue::from_slice(self.builder.inst_results(call))
            }
            Primary::Block(s) => {
                let mut scope_ctx = ScopeContext::new(self.module.make_context());

                let mut binds = self.binds.clone();
                let mut translator =
                    Translator::new(scope_ctx.get_builder(), &mut binds, &mut self.module);
                let ret = translator.translate(&s);

                translator.builder.ins().return_(ret.as_slice());
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
                SpctrValue::from_slice(self.builder.inst_results(call))
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
