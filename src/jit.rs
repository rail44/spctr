use crate::parser::{
    AST,
    Additive,
    AdditiveRight,
    Multitive,
    MultitiveRight
};
use cranelift::prelude::*;
use cranelift_module::{DataContext, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};
use cranelift_codegen::ir::types::*;
use cranelift_codegen::binemit::NullTrapSink;

pub fn translate(ast: &AST) -> *const u8 {
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

    let v = translate_additive(&mut builder, &ast);

    builder.ins().return_(&[v]);
    builder.finalize();

    let id = module
        .declare_function("top", Linkage::Export, &ctx.func.signature)
        .map_err(|e| e.to_string()).unwrap();
    module.define_function(id, &mut ctx, &mut NullTrapSink {}).unwrap();
    module.clear_context(&mut ctx);
    module.finalize_definitions();
    module.get_finalized_function(id)
}

fn translate_additive(builder: &mut FunctionBuilder<'_>, v: &Additive) -> Value {
    let mut lhs = translate_multitive(builder, &v.left);
    for right in &v.rights {
        match right {
            AdditiveRight::Add(r) => {
                let rhs = translate_multitive(builder, &r);
                lhs = builder.ins().fadd(lhs, rhs);
            }
            AdditiveRight::Sub(r) => {
                let rhs = translate_multitive(builder, &r);
                lhs = builder.ins().fsub(lhs, rhs);
            }
        }
    }
    return lhs;
}

fn translate_multitive(builder: &mut FunctionBuilder<'_>, v: &Multitive) -> Value {
    let mut lhs = builder.ins().f64const(v.left);
    for right in &v.rights {
        match right {
            MultitiveRight::Mul(r) => {
                let rhs = builder.ins().f64const(r.clone());
                lhs = builder.ins().fmul(lhs, rhs);
            }
            MultitiveRight::Div(r) => {
                let rhs = builder.ins().f64const(r.clone());
                lhs = builder.ins().fdiv(lhs, rhs);
            }
        }
    }
    return lhs;
}
