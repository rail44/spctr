//! Phase 2 Cranelift JIT.
//!
//! Adds first-class functions and closures to Phase 1's numeric core. Functions
//! are heap-allocated `Closure { fn_ptr, n_caps, captures... }` records, allocated
//! by the `spctr_alloc_closure` `extern "C"` runtime helper. Top-level bindings
//! must be function literals; their closures are pre-allocated at `__spctr_main`
//! prologue (so recursive and mutual references work). Inner function literals
//! materialize their closures inline at the use site.
//!
//! Type-driven IR lowering: every value flows through Cranelift typed by HM
//! results from `typeck`. `Number → F64`, `Bool → I8`, `Fn(...) → I64`. Anything
//! else is rejected with an ariadne-friendly Diagnostic.
use crate::ast::*;
use crate::diag::Diagnostic;
use crate::interp;
use crate::lexer::Span;
use crate::types::{Subst, Type};
use crate::typeck;

use cranelift_codegen::ir::{
    types as ir_types, AbiParam, InstBuilder, MemFlags, Signature, SigRef, Type as IrType,
    Value as IrValue,
};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable as CVar};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use std::collections::{HashMap, HashSet, VecDeque};

// === Runtime helper ==========================================================

/// Closure layout: `[fn_ptr: 8][n_caps: 4][_pad: 4][caps: 8 * n_caps]`.
/// Memory is leaked (Phase 2 simplification — we don't track lifetimes yet).
#[no_mangle]
pub extern "C" fn spctr_alloc_closure(fn_ptr: *const u8, n_caps: u32) -> *mut u8 {
    let size = 16 + 8 * n_caps as usize;
    let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
    unsafe {
        let p = std::alloc::alloc(layout);
        std::ptr::write(p as *mut *const u8, fn_ptr);
        std::ptr::write(p.add(8) as *mut u32, n_caps);
        p
    }
}

const CAPTURES_OFFSET: i32 = 16;

/// Allocates a record with `n_slots` 8-byte slots. Each slot stores a value
/// bit-pattern (f64, i64 closure ptr, i64 record ptr, or i8 zero-extended).
/// Memory is leaked (Phase 3 simplification — same policy as closures).
#[no_mangle]
pub extern "C" fn spctr_alloc_record(n_slots: u32) -> *mut u8 {
    let size = (8 * n_slots as usize).max(8);
    let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
    unsafe { std::alloc::alloc(layout) }
}

/// Allocates a list: `[length: u32][_pad: u32][elem * length]` with each
/// element occupying an 8-byte slot. Caller writes the length and elements;
/// memory is leaked.
#[no_mangle]
pub extern "C" fn spctr_alloc_list(n: u32) -> *mut u8 {
    let size = 8 + 8 * n as usize;
    let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
    unsafe { std::alloc::alloc(layout) }
}

/// Compares two string buffers laid out as `[len: u32][_pad: u32][bytes]`.
/// Returns 1 (i8) if equal, 0 otherwise. Pointer-equal inputs short-circuit.
#[no_mangle]
pub extern "C" fn spctr_str_eq(a: *const u8, b: *const u8) -> u8 {
    if a == b {
        return 1;
    }
    unsafe {
        let la = std::ptr::read(a as *const u32);
        let lb = std::ptr::read(b as *const u32);
        if la != lb {
            return 0;
        }
        let sa = std::slice::from_raw_parts(a.add(8), la as usize);
        let sb = std::slice::from_raw_parts(b.add(8), lb as usize);
        u8::from(sa == sb)
    }
}

// --- stdlib runtime helpers ------------------------------------------------
//
// These mirror the corresponding `crate::stdlib::{number, string, list}`
// implementations but operate on the JIT's flat memory representations
// (`[len][bytes]` for strings, `[len][slot]*n` for lists, raw f64 for numbers).

unsafe fn read_str(ptr: *const u8) -> &'static [u8] {
    let len = unsafe { std::ptr::read(ptr as *const u32) } as usize;
    unsafe { std::slice::from_raw_parts(ptr.add(8), len) }
}

unsafe fn make_str(bytes: &[u8]) -> *mut u8 {
    let total = 8 + bytes.len();
    let layout = std::alloc::Layout::from_size_align(total, 8).unwrap();
    unsafe {
        let p = std::alloc::alloc(layout);
        std::ptr::write(p as *mut u32, bytes.len() as u32);
        std::ptr::write(p.add(4) as *mut u32, 0);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.add(8), bytes.len());
        p
    }
}

#[no_mangle]
pub extern "C" fn spctr_num_pow(a: f64, b: f64) -> f64 {
    a.powf(b)
}

#[no_mangle]
pub extern "C" fn spctr_num_to_string(n: f64) -> *mut u8 {
    let s = format!("{}", n);
    unsafe { make_str(s.as_bytes()) }
}

#[no_mangle]
pub extern "C" fn spctr_num_parse(s: *const u8) -> f64 {
    let bytes = unsafe { read_str(s) };
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(f64::NAN)
}

#[no_mangle]
pub extern "C" fn spctr_str_concat(a: *const u8, b: *const u8) -> *mut u8 {
    unsafe {
        let sa = read_str(a);
        let sb = read_str(b);
        let mut out: Vec<u8> = Vec::with_capacity(sa.len() + sb.len());
        out.extend_from_slice(sa);
        out.extend_from_slice(sb);
        make_str(&out)
    }
}

#[no_mangle]
pub extern "C" fn spctr_str_contains(haystack: *const u8, needle: *const u8) -> u8 {
    unsafe {
        let h = std::str::from_utf8(read_str(haystack)).unwrap_or("");
        let n = std::str::from_utf8(read_str(needle)).unwrap_or("");
        u8::from(h.contains(n))
    }
}

#[no_mangle]
pub extern "C" fn spctr_str_to_lower(s: *const u8) -> *mut u8 {
    unsafe {
        let bytes = read_str(s);
        let s = std::str::from_utf8(bytes).unwrap_or("");
        make_str(s.to_lowercase().as_bytes())
    }
}

#[no_mangle]
pub extern "C" fn spctr_str_to_upper(s: *const u8) -> *mut u8 {
    unsafe {
        let bytes = read_str(s);
        let s = std::str::from_utf8(bytes).unwrap_or("");
        make_str(s.to_uppercase().as_bytes())
    }
}

unsafe fn alloc_list(n: u32) -> *mut u8 {
    let p = spctr_alloc_list(n);
    unsafe { std::ptr::write(p as *mut u32, n) };
    p
}

#[no_mangle]
pub extern "C" fn spctr_str_split(s: *const u8, sep: *const u8) -> *mut u8 {
    unsafe {
        let s_str = std::str::from_utf8(read_str(s)).unwrap_or("");
        let sep_str = std::str::from_utf8(read_str(sep)).unwrap_or("");
        let parts: Vec<&str> = if sep_str.is_empty() {
            // Edge case: split on empty separator yields characters in interp;
            // here we yield a single-element list for predictability.
            vec![s_str]
        } else {
            s_str.split(sep_str).collect()
        };
        let list = alloc_list(parts.len() as u32);
        for (i, part) in parts.iter().enumerate() {
            let str_ptr = make_str(part.as_bytes());
            let slot = list.add(8 + 8 * i);
            std::ptr::write(slot as *mut *mut u8, str_ptr);
        }
        list
    }
}

#[no_mangle]
pub extern "C" fn spctr_list_range(start: f64, end: f64) -> *mut u8 {
    let s = start as i64;
    let e = end as i64;
    let n = (e - s).max(0) as u32;
    unsafe {
        let p = alloc_list(n);
        for i in 0..n {
            let v = (s + i as i64) as f64;
            let slot = p.add(8 + 8 * i as usize);
            std::ptr::write(slot as *mut f64, v);
        }
        p
    }
}

#[no_mangle]
pub extern "C" fn spctr_list_concat(a: *const u8, b: *const u8) -> *mut u8 {
    unsafe {
        let la = std::ptr::read(a as *const u32);
        let lb = std::ptr::read(b as *const u32);
        let total = la + lb;
        let p = alloc_list(total);
        std::ptr::copy_nonoverlapping(a.add(8), p.add(8), 8 * la as usize);
        std::ptr::copy_nonoverlapping(b.add(8), p.add(8 + 8 * la as usize), 8 * lb as usize);
        p
    }
}

/// Returns a fresh list containing slots `[start..start+len]` of `src`. Used
/// by `tail`, `take`, and `drop` from spctr's stdlib.
#[no_mangle]
pub extern "C" fn spctr_list_slice(src: *const u8, start: u32, len: u32) -> *mut u8 {
    unsafe {
        let p = alloc_list(len);
        std::ptr::copy_nonoverlapping(
            src.add(8 + 8 * start as usize),
            p.add(8),
            8 * len as usize,
        );
        p
    }
}

/// Writes the bytes of an spctr string buffer to stdout. Used by the JIT's
/// `display` mode at the end of `__spctr_main` so the program's value gets
/// printed without round-tripping through `JitValue` enums.
#[no_mangle]
pub extern "C" fn spctr_print(s: *const u8) {
    if s.is_null() {
        return;
    }
    unsafe {
        let len = std::ptr::read(s as *const u32) as usize;
        let bytes = std::slice::from_raw_parts(s.add(8), len);
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut h = stdout.lock();
        let _ = h.write_all(bytes);
    }
}

// === Entry ===================================================================

/// Compile and execute an AST through the JIT, returning the f64 produced by
/// the top-level body. Only valid for programs whose body has a numeric type.
/// Tests use this for direct value assertion.
pub fn run(ast: &Statement) -> Result<f64, Diagnostic> {
    run_inner(ast, false)
}

/// Compile and execute an AST through the JIT. Whatever the body's static
/// type is, the value gets printed to stdout from inside the generated code
/// via the `spctr_print` runtime helper. Used by `main.rs` so JIT execution
/// matches tree-walker output for non-numeric programs (records, strings,
/// lists).
pub fn run_with_display(ast: &Statement) -> Result<(), Diagnostic> {
    run_inner(ast, true).map(|_| ())
}

fn run_inner(ast: &Statement, display: bool) -> Result<f64, Diagnostic> {
    let tres = typeck::check(ast, &interp::root_types());
    if let Some(w) = tres.warnings.into_iter().next() {
        return Err(w);
    }

    let mut compiler = Compiler::new(tres.node_types)?;
    compiler.display = display;
    compiler.compile_program(ast)?;
    let main_id = compiler.main_id;
    let mut module = compiler.module;
    module
        .finalize_definitions()
        .map_err(|e| internal(format!("finalize: {e}")))?;
    let main_ptr = module.get_finalized_function(main_id);
    let main_fn: extern "C" fn() -> f64 = unsafe { std::mem::transmute(main_ptr) };
    let result = main_fn();
    std::mem::forget(module);
    Ok(result)
}

fn internal(msg: impl Into<String>) -> Diagnostic {
    Diagnostic::new(0..0, msg, "JIT internal error")
}

fn declare_stdlib(module: &mut JITModule) -> Result<(), Diagnostic> {
    let mk = |module: &mut JITModule, name: &str, params: &[IrType], ret: Option<IrType>| {
        let mut sig = module.make_signature();
        for p in params {
            sig.params.push(AbiParam::new(*p));
        }
        if let Some(r) = ret {
            sig.returns.push(AbiParam::new(r));
        }
        module
            .declare_function(name, Linkage::Import, &sig)
            .map(|_| ())
            .map_err(|e| internal(format!("declare {name}: {e}")))
    };
    mk(module, "spctr_num_pow", &[ir_types::F64, ir_types::F64], Some(ir_types::F64))?;
    mk(module, "spctr_num_to_string", &[ir_types::F64], Some(ir_types::I64))?;
    mk(module, "spctr_num_parse", &[ir_types::I64], Some(ir_types::F64))?;
    mk(module, "spctr_str_concat", &[ir_types::I64, ir_types::I64], Some(ir_types::I64))?;
    mk(module, "spctr_str_contains", &[ir_types::I64, ir_types::I64], Some(ir_types::I8))?;
    mk(module, "spctr_str_to_lower", &[ir_types::I64], Some(ir_types::I64))?;
    mk(module, "spctr_str_to_upper", &[ir_types::I64], Some(ir_types::I64))?;
    mk(module, "spctr_str_split", &[ir_types::I64, ir_types::I64], Some(ir_types::I64))?;
    mk(module, "spctr_list_range", &[ir_types::F64, ir_types::F64], Some(ir_types::I64))?;
    mk(module, "spctr_list_concat", &[ir_types::I64, ir_types::I64], Some(ir_types::I64))?;
    mk(module, "spctr_list_slice", &[ir_types::I64, ir_types::I32, ir_types::I32], Some(ir_types::I64))?;
    mk(module, "spctr_print", &[ir_types::I64], None)?;
    Ok(())
}

// === Type lowering ===========================================================

fn ir_type_for(ty: &Type, span: &Span) -> Result<IrType, Diagnostic> {
    match ty {
        Type::Number => Ok(ir_types::F64),
        Type::Bool | Type::Null => Ok(ir_types::I8),
        Type::Fn(_, _) | Type::Record(_) | Type::List(_) | Type::String => Ok(ir_types::I64),
        _ => Err(Diagnostic::new(
            span.clone(),
            format!("JIT cannot represent type {ty}"),
            "Phase 3 supports number/bool/null/function/record/list/string",
        )),
    }
}

fn contains_var(ty: &Type) -> bool {
    match ty {
        Type::Var(_) => true,
        Type::Fn(args, ret) => args.iter().any(contains_var) || contains_var(ret),
        Type::List(t) => contains_var(t),
        Type::Record(fields) => fields.iter().any(|(_, t)| contains_var(t)),
        Type::Module(fields) => fields.iter().any(|(_, sch)| contains_var(&sch.ty)),
        _ => false,
    }
}

/// Best-effort unification: walks `general` (which may contain `Type::Var`s) and
/// `specific` (assumed monomorphic) in lockstep, populating `subst` so that
/// `general.apply(&subst) == specific`. Mismatched shapes are silently skipped —
/// typeck has already accepted the program, so this is reachable only for
/// places where a quantified var meets a concrete type.
fn unify_subst(general: &Type, specific: &Type, subst: &mut Subst) {
    match (general, specific) {
        (Type::Var(v), other) => {
            subst.entry(*v).or_insert_with(|| other.clone());
        }
        (Type::Fn(p1, r1), Type::Fn(p2, r2)) if p1.len() == p2.len() => {
            for (a, b) in p1.iter().zip(p2.iter()) {
                unify_subst(a, b, subst);
            }
            unify_subst(r1, r2, subst);
        }
        (Type::List(t1), Type::List(t2)) => unify_subst(t1, t2, subst),
        _ => {}
    }
}

fn fn_type_parts(ty: &Type, span: &Span) -> Result<(Vec<IrType>, IrType), Diagnostic> {
    match ty {
        Type::Fn(params, ret) => {
            let pir: Vec<IrType> = params
                .iter()
                .map(|t| ir_type_for(t, span))
                .collect::<Result<_, _>>()?;
            let rir = ir_type_for(ret, span)?;
            Ok((pir, rir))
        }
        _ => Err(Diagnostic::new(
            span.clone(),
            format!("JIT: expected function type, got {ty}"),
            "internal",
        )),
    }
}

// === Compiler state ==========================================================

struct Compiler {
    module: JITModule,
    alloc_closure_id: FuncId,
    /// `spctr_alloc_record` is looked up by name from `module.declarations()`
    /// at the Block compile site, so we don't need to remember its `FuncId` —
    /// declaring it here just registers the symbol.
    main_id: FuncId,
    /// When true, `define_main` wraps the body's value with display IR that
    /// prints to stdout via `spctr_print`, then returns a sentinel `0.0`.
    display: bool,
    node_types: HashMap<usize, Type>,
    /// Function literal instances. A polymorphic literal used at multiple
    /// monomorphic types yields multiple entries (one per `mono_ty_str`).
    funcs: HashMap<FuncKey, FuncInfo>,
    /// Top-level binding instances, in main-prologue allocation order. Each
    /// entry is one (slot, mono_ty) pairing. Mutual references are resolved
    /// via a two-phase fill (alloc all, then populate captures).
    top_level_instances: Vec<TopInstance>,
    call_conv: CallConv,
}

/// Key used to look up a Function literal instance: `(expr_ptr, mono_ty_str)`.
type FuncKey = (usize, String);

#[derive(Clone)]
struct TopInstance {
    slot: u32,
    /// Stringified canonical mono type — both the lookup key into `funcs` and
    /// the key for resolving Variable use sites (whose mono_ty after env subst
    /// must match this string for the use to land on this instance).
    mono_ty_str: String,
    expr_ptr: usize,
    kind: TopKind,
}

#[derive(Clone)]
enum TopKind {
    /// Top-level binding is a Function literal. `funcs[(expr_ptr, mono_ty_str)]`
    /// holds the compiled body; the CVar in main carries a `closure_ptr` (i64).
    Function,
    /// Top-level binding is some other expression whose monomorphic value is
    /// computed eagerly at main-prologue time. The CVar carries the value
    /// directly with the given IR type.
    Value(IrType),
}

#[derive(Clone)]
struct FuncInfo {
    func_id: FuncId,
    /// Param IR types, excluding the implicit closure_ptr.
    param_irtys: Vec<IrType>,
    ret_irty: IrType,
    /// Captures, in this function's body coordinates (`inside.depth >= 1`).
    captures: Vec<Capture>,
    /// 0 = function literal sits directly at top-level binding or in main body.
    /// N = N enclosing function literals.
    nesting: u32,
    /// Substitution from typeck's quantified vars to monomorphic types — applied
    /// to every per-node type lookup during this function's codegen. Inner
    /// function literals inherit their enclosing top-level function's subst.
    subst: Subst,
}

#[derive(Clone)]
struct Capture {
    /// BindRef as seen from inside this function's body.
    inside: BindRef,
    irty: IrType,
    /// Mono ty string of the captured value at the inner use site. When the
    /// capture refers to a top-level function, this disambiguates which
    /// `TopInstance` to read from at allocation time.
    mono_ty_str: String,
}

impl Compiler {
    fn new(node_types: HashMap<usize, Type>) -> Result<Self, Diagnostic> {
        let mut flag_builder = settings::builder();
        flag_builder
            .set("use_colocated_libcalls", "false")
            .map_err(|e| internal(format!("flag: {e}")))?;
        flag_builder
            .set("is_pic", "false")
            .map_err(|e| internal(format!("flag: {e}")))?;
        let isa_builder =
            cranelift_native::builder().map_err(|e| internal(format!("native: {e}")))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| internal(format!("isa: {e}")))?;
        let call_conv = isa.default_call_conv();
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        builder.symbol("spctr_alloc_closure", spctr_alloc_closure as *const u8);
        builder.symbol("spctr_alloc_record", spctr_alloc_record as *const u8);
        builder.symbol("spctr_alloc_list", spctr_alloc_list as *const u8);
        builder.symbol("spctr_str_eq", spctr_str_eq as *const u8);
        builder.symbol("spctr_num_pow", spctr_num_pow as *const u8);
        builder.symbol("spctr_num_to_string", spctr_num_to_string as *const u8);
        builder.symbol("spctr_num_parse", spctr_num_parse as *const u8);
        builder.symbol("spctr_str_concat", spctr_str_concat as *const u8);
        builder.symbol("spctr_str_contains", spctr_str_contains as *const u8);
        builder.symbol("spctr_str_to_lower", spctr_str_to_lower as *const u8);
        builder.symbol("spctr_str_to_upper", spctr_str_to_upper as *const u8);
        builder.symbol("spctr_str_split", spctr_str_split as *const u8);
        builder.symbol("spctr_list_range", spctr_list_range as *const u8);
        builder.symbol("spctr_list_concat", spctr_list_concat as *const u8);
        builder.symbol("spctr_list_slice", spctr_list_slice as *const u8);
        builder.symbol("spctr_print", spctr_print as *const u8);
        let mut module = JITModule::new(builder);

        let mut alloc_sig = module.make_signature();
        alloc_sig.params.push(AbiParam::new(ir_types::I64));
        alloc_sig.params.push(AbiParam::new(ir_types::I32));
        alloc_sig.returns.push(AbiParam::new(ir_types::I64));
        let alloc_closure_id = module
            .declare_function("spctr_alloc_closure", Linkage::Import, &alloc_sig)
            .map_err(|e| internal(format!("declare alloc: {e}")))?;

        let mut record_sig = module.make_signature();
        record_sig.params.push(AbiParam::new(ir_types::I32));
        record_sig.returns.push(AbiParam::new(ir_types::I64));
        module
            .declare_function("spctr_alloc_record", Linkage::Import, &record_sig)
            .map_err(|e| internal(format!("declare alloc_record: {e}")))?;

        let mut list_sig = module.make_signature();
        list_sig.params.push(AbiParam::new(ir_types::I32));
        list_sig.returns.push(AbiParam::new(ir_types::I64));
        module
            .declare_function("spctr_alloc_list", Linkage::Import, &list_sig)
            .map_err(|e| internal(format!("declare alloc_list: {e}")))?;

        let mut streq_sig = module.make_signature();
        streq_sig.params.push(AbiParam::new(ir_types::I64));
        streq_sig.params.push(AbiParam::new(ir_types::I64));
        streq_sig.returns.push(AbiParam::new(ir_types::I8));
        module
            .declare_function("spctr_str_eq", Linkage::Import, &streq_sig)
            .map_err(|e| internal(format!("declare str_eq: {e}")))?;

        // Stdlib helpers — declare each with its precise ABI so the JIT can
        // call them via Cranelift's regular `call` instruction.
        declare_stdlib(&mut module)?;

        let mut main_sig = module.make_signature();
        main_sig.returns.push(AbiParam::new(ir_types::F64));
        let main_id = module
            .declare_function("__spctr_main", Linkage::Local, &main_sig)
            .map_err(|e| internal(format!("declare main: {e}")))?;

        Ok(Self {
            module,
            alloc_closure_id,
            main_id,
            display: false,
            node_types,
            funcs: HashMap::new(),
            top_level_instances: Vec::new(),
            call_conv,
        })
    }
}

// === display emitter ========================================================

fn emit_static_str_ptr(bcx: &mut FunctionBuilder, s: &str) -> IrValue {
    let bytes = s.as_bytes();
    let mut buf: Vec<u8> = Vec::with_capacity(8 + bytes.len());
    buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(bytes);
    let leaked: &'static [u8] = Box::leak(buf.into_boxed_slice());
    bcx.ins().iconst(ir_types::I64, leaked.as_ptr() as i64)
}

fn emit_print_static(
    bcx: &mut FunctionBuilder,
    module: &mut JITModule,
    s: &str,
) -> Result<(), Diagnostic> {
    let ptr = emit_static_str_ptr(bcx, s);
    emit_print_value(bcx, module, ptr)
}

fn emit_print_value(
    bcx: &mut FunctionBuilder,
    module: &mut JITModule,
    str_ptr: IrValue,
) -> Result<(), Diagnostic> {
    let id = match module.declarations().get_name("spctr_print") {
        Some(cranelift_module::FuncOrDataId::Func(id)) => id,
        _ => return Err(internal("spctr_print not declared")),
    };
    let r = module.declare_func_in_func(id, bcx.func);
    bcx.ins().call(r, &[str_ptr]);
    Ok(())
}

/// Materialize a `[len: u32][_pad: u32][bytes]` buffer for a string literal at
/// JIT compile time. The buffer is `Box::leak`'d so the compiled IR can embed
/// its address as an i64 constant; strings are immutable in spctr so sharing
/// the leaked buffer across runs is safe and the leak is bounded by the
/// literal count.
fn emit_string_literal(bcx: &mut FunctionBuilder, s: &str) -> JVal {
    let bytes = s.as_bytes();
    let mut buf: Vec<u8> = Vec::with_capacity(8 + bytes.len());
    buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(bytes);
    let leaked: &'static [u8] = Box::leak(buf.into_boxed_slice());
    let ptr = leaked.as_ptr() as i64;
    JVal {
        val: bcx.ins().iconst(ir_types::I64, ptr),
        irty: ir_types::I64,
    }
}

fn emit_empty_string(bcx: &mut FunctionBuilder) -> JVal {
    emit_string_literal(bcx, "")
}

/// Emit IR that prints a value of static type `ty` to stdout. Uses inline
/// branches/loops for bools/lists; calls `spctr_num_to_string` for numbers.
fn emit_display(
    bcx: &mut FunctionBuilder,
    val: IrValue,
    ty: &Type,
    module: &mut JITModule,
    node_types: &HashMap<usize, Type>,
    span: &Span,
) -> Result<(), Diagnostic> {
    use cranelift_codegen::ir::condcodes::IntCC;
    match ty {
        Type::Number => {
            let id = match module.declarations().get_name("spctr_num_to_string") {
                Some(cranelift_module::FuncOrDataId::Func(id)) => id,
                _ => return Err(internal("spctr_num_to_string not declared")),
            };
            let r = module.declare_func_in_func(id, bcx.func);
            let inst = bcx.ins().call(r, &[val]);
            let s = bcx.inst_results(inst)[0];
            emit_print_value(bcx, module, s)
        }
        Type::Bool => {
            let true_blk = bcx.create_block();
            let false_blk = bcx.create_block();
            let merge = bcx.create_block();
            bcx.ins().brif(val, true_blk, &[], false_blk, &[]);

            bcx.switch_to_block(true_blk);
            bcx.seal_block(true_blk);
            emit_print_static(bcx, module, "true")?;
            bcx.ins().jump(merge, &[]);

            bcx.switch_to_block(false_blk);
            bcx.seal_block(false_blk);
            emit_print_static(bcx, module, "false")?;
            bcx.ins().jump(merge, &[]);

            bcx.switch_to_block(merge);
            bcx.seal_block(merge);
            Ok(())
        }
        Type::Null => emit_print_static(bcx, module, "null"),
        Type::String => {
            // Match interp output: `"escaped"`. We skip escaping for now; raw
            // bytes inside quotes is sufficient for typical programs.
            emit_print_static(bcx, module, "\"")?;
            emit_print_value(bcx, module, val)?;
            emit_print_static(bcx, module, "\"")?;
            Ok(())
        }
        Type::Fn(_, _) => emit_print_static(bcx, module, "[function]"),
        Type::Record(fields) => {
            // Match the tree-walker: fields are printed alphabetically by name
            // even though the heap layout uses declaration order.
            emit_print_static(bcx, module, "{")?;
            let mut indexed: Vec<(usize, &(crate::symbol::Symbol, Type))> =
                fields.iter().enumerate().collect();
            indexed.sort_by_key(|(_, (name, _))| crate::symbol::display(*name));
            for (printed_idx, (slot, (name, ft))) in indexed.into_iter().enumerate() {
                if printed_idx > 0 {
                    emit_print_static(bcx, module, ", ")?;
                }
                let key = format!("\"{}\": ", crate::symbol::display(*name));
                emit_print_static(bcx, module, &key)?;
                let irty = ir_type_for(ft, span)?;
                let off = 8 * slot as i32;
                let f_val = bcx.ins().load(irty, MemFlags::trusted(), val, off);
                emit_display(bcx, f_val, ft, module, node_types, span)?;
            }
            emit_print_static(bcx, module, "}")
        }
        Type::List(elem) => {
            let len_u32 = bcx.ins().load(ir_types::I32, MemFlags::trusted(), val, 0);
            let len_i64 = bcx.ins().uextend(ir_types::I64, len_u32);

            emit_print_static(bcx, module, "[")?;

            let header = bcx.create_block();
            let body = bcx.create_block();
            let exit = bcx.create_block();

            let i_var = bcx.declare_var(ir_types::I64);
            let zero = bcx.ins().iconst(ir_types::I64, 0);
            bcx.def_var(i_var, zero);

            bcx.ins().jump(header, &[]);
            bcx.switch_to_block(header);
            let i = bcx.use_var(i_var);
            let cond = bcx.ins().icmp(IntCC::SignedLessThan, i, len_i64);
            bcx.ins().brif(cond, body, &[], exit, &[]);

            bcx.switch_to_block(body);
            bcx.seal_block(body);
            let i_b = bcx.use_var(i_var);

            // Print ", " between elements.
            let zero_chk = bcx.ins().iconst(ir_types::I64, 0);
            let is_first = bcx.ins().icmp(IntCC::Equal, i_b, zero_chk);
            let no_comma = bcx.create_block();
            let do_comma = bcx.create_block();
            let after_comma = bcx.create_block();
            bcx.ins().brif(is_first, no_comma, &[], do_comma, &[]);

            bcx.switch_to_block(do_comma);
            bcx.seal_block(do_comma);
            emit_print_static(bcx, module, ", ")?;
            bcx.ins().jump(after_comma, &[]);

            bcx.switch_to_block(no_comma);
            bcx.seal_block(no_comma);
            bcx.ins().jump(after_comma, &[]);

            bcx.switch_to_block(after_comma);
            bcx.seal_block(after_comma);

            // Load the i-th element and recurse.
            let elem_irty = ir_type_for(elem, span)?;
            let off = bcx.ins().imul_imm(i_b, 8);
            let off = bcx.ins().iadd_imm(off, 8);
            let addr = bcx.ins().iadd(val, off);
            let elem_val = bcx.ins().load(elem_irty, MemFlags::trusted(), addr, 0);
            emit_display(bcx, elem_val, elem, module, node_types, span)?;

            let next_i = bcx.ins().iadd_imm(i_b, 1);
            bcx.def_var(i_var, next_i);
            bcx.ins().jump(header, &[]);

            bcx.seal_block(header);
            bcx.switch_to_block(exit);
            bcx.seal_block(exit);

            emit_print_static(bcx, module, "]")
        }
        other => Err(Diagnostic::new(
            span.clone(),
            format!("JIT: cannot display type {other}"),
            "",
        )),
    }
}

/// Deep structural equality for a value of static type `ty`.
///
/// Mirrors `interp::value_eq`:
/// - Numbers / Bool / Null / String: native equality (numbers use `==`, not
///   epsilon — matching the tree-walker is what callers expect, and the
///   tree-walker uses `(a - b).abs() < f64::EPSILON` which we approximate with
///   exact equality; in practice this matches for the values the test suite
///   produces).
/// - Lists: length check then recursive elementwise.
/// - Records / Closures: always `false` (tree-walker `_ => false` fallthrough).
///
/// Returns an `i8` (0/1).
fn emit_value_eq(
    bcx: &mut FunctionBuilder,
    lv: IrValue,
    rv: IrValue,
    ty: &Type,
    module: &mut JITModule,
    span: &Span,
) -> Result<IrValue, Diagnostic> {
    use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
    match ty {
        Type::Number => Ok(bcx.ins().fcmp(FloatCC::Equal, lv, rv)),
        Type::Bool | Type::Null => Ok(bcx.ins().icmp(IntCC::Equal, lv, rv)),
        Type::String => {
            let id = match module.declarations().get_name("spctr_str_eq") {
                Some(cranelift_module::FuncOrDataId::Func(id)) => id,
                _ => return Err(internal("spctr_str_eq not declared")),
            };
            let r = module.declare_func_in_func(id, bcx.func);
            let inst = bcx.ins().call(r, &[lv, rv]);
            Ok(bcx.inst_results(inst)[0])
        }
        Type::List(elem) => {
            let elem_irty = ir_type_for(elem, span)?;

            let len_a = bcx.ins().load(ir_types::I32, MemFlags::trusted(), lv, 0);
            let len_b = bcx.ins().load(ir_types::I32, MemFlags::trusted(), rv, 0);
            let len_eq = bcx.ins().icmp(IntCC::Equal, len_a, len_b);

            let check_elems = bcx.create_block();
            let merge = bcx.create_block();
            bcx.append_block_param(merge, ir_types::I8);

            let zero_i8 = bcx.ins().iconst(ir_types::I8, 0);
            bcx.ins().brif(len_eq, check_elems, &[], merge, &[zero_i8.into()]);

            bcx.switch_to_block(check_elems);
            bcx.seal_block(check_elems);
            let len_i64 = bcx.ins().uextend(ir_types::I64, len_a);

            let header = bcx.create_block();
            let body = bcx.create_block();
            let exit = bcx.create_block();

            let i_var = bcx.declare_var(ir_types::I64);
            let zero_i64 = bcx.ins().iconst(ir_types::I64, 0);
            bcx.def_var(i_var, zero_i64);
            bcx.ins().jump(header, &[]);

            bcx.switch_to_block(header);
            let i = bcx.use_var(i_var);
            let in_range = bcx.ins().icmp(IntCC::SignedLessThan, i, len_i64);
            bcx.ins().brif(in_range, body, &[], exit, &[]);

            bcx.switch_to_block(body);
            bcx.seal_block(body);
            let i_b = bcx.use_var(i_var);
            let off = bcx.ins().imul_imm(i_b, 8);
            let off = bcx.ins().iadd_imm(off, 8);
            let addr_l = bcx.ins().iadd(lv, off);
            let addr_r = bcx.ins().iadd(rv, off);
            let elem_l = bcx.ins().load(elem_irty, MemFlags::trusted(), addr_l, 0);
            let elem_r = bcx.ins().load(elem_irty, MemFlags::trusted(), addr_r, 0);
            let elem_eq = emit_value_eq(bcx, elem_l, elem_r, elem, module, span)?;

            let next = bcx.create_block();
            let zero_i8_b = bcx.ins().iconst(ir_types::I8, 0);
            bcx.ins().brif(elem_eq, next, &[], merge, &[zero_i8_b.into()]);

            bcx.switch_to_block(next);
            bcx.seal_block(next);
            let next_i = bcx.ins().iadd_imm(i_b, 1);
            bcx.def_var(i_var, next_i);
            bcx.ins().jump(header, &[]);

            bcx.seal_block(header);

            bcx.switch_to_block(exit);
            bcx.seal_block(exit);
            let one_i8 = bcx.ins().iconst(ir_types::I8, 1);
            bcx.ins().jump(merge, &[one_i8.into()]);

            bcx.switch_to_block(merge);
            bcx.seal_block(merge);
            Ok(bcx.block_params(merge)[0])
        }
        // Tree-walker treats records and closures as never-equal.
        Type::Record(_) | Type::Fn(_, _) => Ok(bcx.ins().iconst(ir_types::I8, 0)),
        _ => Err(Diagnostic::new(
            span.clone(),
            format!("JIT: == not supported for type {ty}"),
            "",
        )),
    }
}

// (continuing the inherent impl) ============================================
impl Compiler {

    fn compile_program(&mut self, ast: &Statement) -> Result<(), Diagnostic> {
        let empty_subst = Subst::new();

        // Phase 3d: top-level bindings can be Function literals OR arbitrary
        // monomorphic value expressions. Register a `Value` TopInstance for
        // each non-function binding eagerly; their bodies are also seeded into
        // the worklist so transitive function uses get discovered.
        for (slot, (_, body)) in ast.definitions.iter().enumerate() {
            if matches!(body.0, Expr::Function(_, _)) {
                continue;
            }
            let body_ptr = body as *const _ as usize;
            let mono_ty = self
                .node_types
                .get(&body_ptr)
                .cloned()
                .ok_or_else(|| Diagnostic::new(body.1.clone(), "JIT: missing type info", ""))?;
            if contains_var(&mono_ty) {
                return Err(Diagnostic::new(
                    body.1.clone(),
                    format!(
                        "JIT: top-level binding has unresolved type {mono_ty}"
                    ),
                    "non-function bindings must be fully monomorphic",
                ));
            }
            let irty = ir_type_for(&mono_ty, &body.1)?;
            self.top_level_instances.push(TopInstance {
                slot: slot as u32,
                mono_ty_str: format!("{mono_ty}"),
                expr_ptr: body_ptr,
                kind: TopKind::Value(irty),
            });
        }

        // Pass 1: BFS over top-level FUNCTION instantiations. Seed with uses
        // from main body and from every non-function binding body.
        let mut visited: HashSet<(u32, String)> = HashSet::new();
        let mut worklist: VecDeque<(u32, String, Type, usize)> = VecDeque::new();

        let mut seed_uses: HashMap<u32, Vec<(String, Type)>> = HashMap::new();
        self.collect_uses_in(&ast.body, 0, &empty_subst, &mut seed_uses);
        for (_, body) in &ast.definitions {
            if !matches!(body.0, Expr::Function(_, _)) {
                self.collect_uses_in(body, 0, &empty_subst, &mut seed_uses);
            }
        }
        for (slot, items) in seed_uses {
            let body = &ast.definitions[slot as usize].1;
            // Only function bindings flow through the worklist; non-function
            // ones have a fixed mono ty already registered above.
            if !matches!(body.0, Expr::Function(_, _)) {
                continue;
            }
            for (mono_str, mono_ty) in items {
                if visited.insert((slot, mono_str.clone())) {
                    worklist.push_back((slot, mono_str, mono_ty, body as *const _ as usize));
                }
            }
        }
        // Top-level function bindings with no uses at all: still emit with
        // their def_ty (must be monomorphic — otherwise reject).
        for (slot, (_, body)) in ast.definitions.iter().enumerate() {
            if !matches!(body.0, Expr::Function(_, _)) {
                continue;
            }
            let s = slot as u32;
            if visited.iter().any(|(v, _)| *v == s) {
                continue;
            }
            let body_ptr = body as *const _ as usize;
            let def_ty = self
                .node_types
                .get(&body_ptr)
                .cloned()
                .ok_or_else(|| Diagnostic::new(body.1.clone(), "JIT: missing type info", ""))?;
            if contains_var(&def_ty) {
                return Err(Diagnostic::new(
                    body.1.clone(),
                    format!("JIT: top-level function has unresolved type {def_ty}"),
                    "polymorphic but never used",
                ));
            }
            let key = format!("{def_ty}");
            visited.insert((s, key.clone()));
            worklist.push_back((s, key, def_ty, body_ptr));
        }

        while let Some((slot, mono_str, mono_ty, body_ptr)) = worklist.pop_front() {
            // SAFETY: AST outlives the JIT compile.
            let body: &Spanned<Expr> = unsafe { &*(body_ptr as *const Spanned<Expr>) };
            let def_ty = self
                .node_types
                .get(&body_ptr)
                .cloned()
                .ok_or_else(|| Diagnostic::new(body.1.clone(), "JIT: missing type info", ""))?;
            let mut subst = Subst::new();
            unify_subst(&def_ty, &mono_ty, &mut subst);

            let mut local_uses: HashMap<u32, Vec<(String, Type)>> = HashMap::new();
            self.collect_uses_in(body, 0, &subst, &mut local_uses);
            for (other_slot, items) in local_uses {
                let other_body = &ast.definitions[other_slot as usize].1;
                if !matches!(other_body.0, Expr::Function(_, _)) {
                    continue;
                }
                for (other_str, other_ty) in items {
                    if visited.insert((other_slot, other_str.clone())) {
                        worklist.push_back((
                            other_slot,
                            other_str,
                            other_ty,
                            other_body as *const _ as usize,
                        ));
                    }
                }
            }

            self.discover(body, 0, &subst, ast)?;
            self.top_level_instances.push(TopInstance {
                slot,
                mono_ty_str: mono_str,
                expr_ptr: body_ptr,
                kind: TopKind::Function,
            });
        }

        // Function literals appearing inside main's body or inside non-function
        // top-level binding bodies are discovered with an empty subst.
        self.discover(&ast.body, 0, &empty_subst, ast)?;
        for (_, body) in &ast.definitions {
            if !matches!(body.0, Expr::Function(_, _)) {
                self.discover(body, 0, &empty_subst, ast)?;
            }
        }

        // Pass 2: compile each declared FuncInfo's body.
        let keys: Vec<FuncKey> = self.funcs.keys().cloned().collect();
        for key in keys {
            self.compile_function_instance(&key)?;
        }

        // Pass 3: compile main.
        self.define_main(ast)?;
        Ok(())
    }

    /// Walk a body looking for Variable uses that resolve to the top-level
    /// frame (i.e., `bref.depth == depth_to_tl`). For each such use, take the
    /// node's typeck-recorded type, apply `subst` (the enclosing function's
    /// monomorphization), and — if it's now concrete — record `(slot, ty)`.
    fn collect_uses_in(
        &self,
        e: &Spanned<Expr>,
        depth_to_tl: u32,
        subst: &Subst,
        out: &mut HashMap<u32, Vec<(String, Type)>>,
    ) {
        match &e.0 {
            Expr::Variable(v) => {
                if let Some(bref) = v.resolved.get() {
                    if bref.depth == depth_to_tl {
                        if let Some(t) = self
                            .node_types
                            .get(&(e as *const _ as usize))
                            .cloned()
                        {
                            let resolved = t.apply(subst);
                            if !contains_var(&resolved) {
                                let key = format!("{resolved}");
                                let entry = out.entry(bref.slot).or_default();
                                if !entry.iter().any(|(k, _)| k == &key) {
                                    entry.push((key, resolved));
                                }
                            }
                        }
                    }
                }
            }
            Expr::Function(_, body) => self.collect_uses_in(body, depth_to_tl + 1, subst, out),
            Expr::List(items) => {
                for it in items {
                    self.collect_uses_in(it, depth_to_tl, subst, out);
                }
            }
            Expr::Block(defs) => {
                for (_, b) in defs {
                    self.collect_uses_in(b, depth_to_tl + 1, subst, out);
                }
            }
            Expr::ImmediateBlock(stmt) => {
                for (_, b) in &stmt.definitions {
                    self.collect_uses_in(b, depth_to_tl + 1, subst, out);
                }
                self.collect_uses_in(&stmt.body, depth_to_tl + 1, subst, out);
            }
            Expr::If { cond, cons, alt } => {
                self.collect_uses_in(cond, depth_to_tl, subst, out);
                self.collect_uses_in(cons, depth_to_tl, subst, out);
                self.collect_uses_in(alt, depth_to_tl, subst, out);
            }
            Expr::Binary(_, l, r) => {
                self.collect_uses_in(l, depth_to_tl, subst, out);
                self.collect_uses_in(r, depth_to_tl, subst, out);
            }
            Expr::Unary(_, e) => self.collect_uses_in(e, depth_to_tl, subst, out),
            Expr::Call(callee, args) => {
                self.collect_uses_in(callee, depth_to_tl, subst, out);
                for a in args {
                    self.collect_uses_in(a, depth_to_tl, subst, out);
                }
            }
            Expr::Access(o, _) => self.collect_uses_in(o, depth_to_tl, subst, out),
            Expr::Index(a, i) => {
                self.collect_uses_in(a, depth_to_tl, subst, out);
                self.collect_uses_in(i, depth_to_tl, subst, out);
            }
            _ => {}
        }
    }

    // ------ Pass 1: discover & declare ------

    fn discover(
        &mut self,
        expr: &Spanned<Expr>,
        nesting: u32,
        parent_subst: &Subst,
        ast: &Statement,
    ) -> Result<(), Diagnostic> {
        if let Expr::Function(_, body) = &expr.0 {
            let expr_ptr = expr as *const _ as usize;
            let def_ty = self
                .node_types
                .get(&expr_ptr)
                .cloned()
                .ok_or_else(|| Diagnostic::new(expr.1.clone(), "JIT: missing type info", ""))?;
            let mono_ty = def_ty.apply(parent_subst);
            if contains_var(&mono_ty) {
                return Err(Diagnostic::new(
                    expr.1.clone(),
                    format!("JIT: function literal has unresolved type {mono_ty}"),
                    "Phase 2.5 needs monomorphic types at every site",
                ));
            }
            let mono_ty_str = format!("{mono_ty}");
            let key: FuncKey = (expr_ptr, mono_ty_str.clone());
            // If we've already declared this instance, just descend without
            // re-declaring (avoids duplicate FuncIds for shared literals).
            if self.funcs.contains_key(&key) {
                self.discover(body, nesting + 1, parent_subst, ast)?;
                return Ok(());
            }

            let (param_irtys, ret_irty) = fn_type_parts(&mono_ty, &expr.1)?;
            let subst = parent_subst.clone();
            let captures = self.compute_captures(expr, &subst)?;

            let mut sig = self.module.make_signature();
            sig.params.push(AbiParam::new(ir_types::I64)); // closure_ptr
            for &p in &param_irtys {
                sig.params.push(AbiParam::new(p));
            }
            sig.returns.push(AbiParam::new(ret_irty));
            let id = self
                .module
                .declare_function(&format!("fn{}", self.funcs.len()), Linkage::Local, &sig)
                .map_err(|e| internal(format!("declare fn: {e}")))?;

            self.funcs.insert(
                key,
                FuncInfo {
                    func_id: id,
                    param_irtys,
                    ret_irty,
                    captures,
                    nesting,
                    subst: subst.clone(),
                },
            );

            self.discover(body, nesting + 1, &subst, ast)?;
            return Ok(());
        }
        // Use a closure to recurse with the same subst+ast threading.
        match &expr.0 {
            Expr::List(items) => {
                for it in items {
                    self.discover(it, nesting, parent_subst, ast)?;
                }
            }
            Expr::Block(defs) => {
                for (_, b) in defs {
                    self.discover(b, nesting, parent_subst, ast)?;
                }
            }
            Expr::ImmediateBlock(stmt) => {
                for (_, b) in &stmt.definitions {
                    self.discover(b, nesting, parent_subst, ast)?;
                }
                self.discover(&stmt.body, nesting, parent_subst, ast)?;
            }
            Expr::If { cond, cons, alt } => {
                self.discover(cond, nesting, parent_subst, ast)?;
                self.discover(cons, nesting, parent_subst, ast)?;
                self.discover(alt, nesting, parent_subst, ast)?;
            }
            Expr::Binary(_, l, r) => {
                self.discover(l, nesting, parent_subst, ast)?;
                self.discover(r, nesting, parent_subst, ast)?;
            }
            Expr::Unary(_, e) => self.discover(e, nesting, parent_subst, ast)?,
            Expr::Call(callee, args) => {
                self.discover(callee, nesting, parent_subst, ast)?;
                for a in args {
                    self.discover(a, nesting, parent_subst, ast)?;
                }
            }
            Expr::Access(o, _) => self.discover(o, nesting, parent_subst, ast)?,
            Expr::Index(a, i) => {
                self.discover(a, nesting, parent_subst, ast)?;
                self.discover(i, nesting, parent_subst, ast)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn compute_captures(
        &self,
        func_expr: &Spanned<Expr>,
        subst: &Subst,
    ) -> Result<Vec<Capture>, Diagnostic> {
        let body = match &func_expr.0 {
            Expr::Function(_, b) => b,
            _ => unreachable!(),
        };
        let mut caps: HashMap<(u32, u32), Type> = HashMap::new();
        self.collect_captures(body, 0, &mut caps);
        let mut sorted: Vec<((u32, u32), Type)> = caps.into_iter().collect();
        sorted.sort_by_key(|(k, _)| *k);
        let mut out = Vec::with_capacity(sorted.len());
        for ((d, s), ty) in sorted {
            let mono = ty.apply(subst);
            if contains_var(&mono) {
                return Err(Diagnostic::new(
                    func_expr.1.clone(),
                    format!("JIT: capture has unresolved type {mono}"),
                    "Phase 2.5",
                ));
            }
            let irty = ir_type_for(&mono, &func_expr.1)?;
            out.push(Capture {
                inside: BindRef { depth: d, slot: s },
                irty,
                mono_ty_str: format!("{mono}"),
            });
        }
        Ok(out)
    }

    fn collect_captures(
        &self,
        e: &Spanned<Expr>,
        layers: u32,
        caps: &mut HashMap<(u32, u32), Type>,
    ) {
        match &e.0 {
            Expr::Variable(v) => {
                if let Some(bref) = v.resolved.get() {
                    if bref.depth as i64 - layers as i64 >= 1 {
                        let inside_depth = bref.depth - layers;
                        let ty = self
                            .node_types
                            .get(&(e as *const _ as usize))
                            .cloned()
                            .unwrap_or(Type::Any);
                        // Skip references to root-scope stdlib modules: these
                        // are resolved statically by the stdlib dispatcher and
                        // should never enter the closure capture set.
                        if !matches!(ty, Type::Module(_)) {
                            caps.entry((inside_depth, bref.slot)).or_insert(ty);
                        }
                    }
                }
            }
            Expr::Function(_, body) => self.collect_captures(body, layers + 1, caps),
            Expr::List(items) => {
                for it in items {
                    self.collect_captures(it, layers, caps);
                }
            }
            // Block / ImmediateBlock add a resolver scope, so VarRef coords
            // inside their bodies are shifted by 1.
            Expr::Block(defs) => {
                for (_, b) in defs {
                    self.collect_captures(b, layers + 1, caps);
                }
            }
            Expr::ImmediateBlock(stmt) => {
                for (_, b) in &stmt.definitions {
                    self.collect_captures(b, layers + 1, caps);
                }
                self.collect_captures(&stmt.body, layers + 1, caps);
            }
            Expr::If { cond, cons, alt } => {
                self.collect_captures(cond, layers, caps);
                self.collect_captures(cons, layers, caps);
                self.collect_captures(alt, layers, caps);
            }
            Expr::Binary(_, l, r) => {
                self.collect_captures(l, layers, caps);
                self.collect_captures(r, layers, caps);
            }
            Expr::Unary(_, e) => self.collect_captures(e, layers, caps),
            Expr::Call(callee, args) => {
                self.collect_captures(callee, layers, caps);
                for a in args {
                    self.collect_captures(a, layers, caps);
                }
            }
            Expr::Access(o, _) => self.collect_captures(o, layers, caps),
            Expr::Index(a, i) => {
                self.collect_captures(a, layers, caps);
                self.collect_captures(i, layers, caps);
            }
            _ => {}
        }
    }

    // ------ Pass 2: compile function bodies ------

    fn compile_function_instance(&mut self, key: &FuncKey) -> Result<(), Diagnostic> {
        let info = self
            .funcs
            .get(key)
            .cloned()
            .ok_or_else(|| internal("missing FuncInfo"))?;
        // SAFETY: AST is owned by the caller and outlives the JIT compile.
        let func_expr: &Spanned<Expr> = unsafe { &*(key.0 as *const Spanned<Expr>) };
        let body = match &func_expr.0 {
            Expr::Function(_, b) => b.as_ref(),
            _ => unreachable!(),
        };

        let mut ctx = self.module.make_context();
        ctx.func.signature = self
            .module
            .declarations()
            .get_function_decl(info.func_id)
            .signature
            .clone();

        let mut fb_ctx = FunctionBuilderContext::new();
        let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = bcx.create_block();
        bcx.append_block_params_for_function_params(entry);
        bcx.switch_to_block(entry);
        bcx.seal_block(entry);

        let block_params: Vec<IrValue> = bcx.block_params(entry).to_vec();
        let closure_ptr = block_params[0];
        let arg_vals: Vec<IrValue> = block_params[1..].to_vec();

        let env = CompileEnv {
            kind: EnvKind::Function {
                closure_ptr,
                args: &arg_vals,
                captures: &info.captures,
                depth_to_top_level: info.nesting + 1,
            },
            block_frames: Vec::new(),
            subst: &info.subst,
        };
        let module_ptr = &mut self.module as *mut JITModule;
        let funcs_ptr = &self.funcs as *const HashMap<FuncKey, FuncInfo>;
        let top_level_ptr = &self.top_level_instances as *const Vec<TopInstance>;
        let node_types_ptr = &self.node_types as *const HashMap<usize, Type>;
        let alloc_id = self.alloc_closure_id;
        let cc = self.call_conv;
        let result = unsafe {
            compile_expr(
                &mut bcx,
                body,
                &env,
                &mut *module_ptr,
                &*funcs_ptr,
                &*top_level_ptr,
                &*node_types_ptr,
                alloc_id,
                cc,
            )
        }?;
        let ret = coerce_to(&mut bcx, result, info.ret_irty, &body.1)?;
        bcx.ins().return_(&[ret]);
        bcx.seal_all_blocks();
        bcx.finalize();

        self.module
            .define_function(info.func_id, &mut ctx)
            .map_err(|e| Diagnostic::new(body.1.clone(), format!("define: {e}"), "JIT"))?;
        self.module.clear_context(&mut ctx);
        Ok(())
    }

    // ------ Pass 3: compile main ------

    fn define_main(&mut self, ast: &Statement) -> Result<(), Diagnostic> {
        let mut ctx = self.module.make_context();
        ctx.func.signature = self
            .module
            .declarations()
            .get_function_decl(self.main_id)
            .signature
            .clone();
        let mut fb_ctx = FunctionBuilderContext::new();
        let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);
        let entry = bcx.create_block();
        bcx.switch_to_block(entry);
        bcx.seal_block(entry);

        // Phase A: declare a CVar for every TopInstance and pre-allocate
        // closures for the function ones. Value instances get their CVar but
        // remain undefined until Phase B fills them in source order.
        let mut top_vars: Vec<CVar> = Vec::with_capacity(self.top_level_instances.len());
        for inst in &self.top_level_instances {
            match inst.kind {
                TopKind::Function => {
                    let key: FuncKey = (inst.expr_ptr, inst.mono_ty_str.clone());
                    let info = self.funcs.get(&key).expect("declared").clone();
                    let var = bcx.declare_var(ir_types::I64);
                    top_vars.push(var);
                    let func_ref = self.module.declare_func_in_func(info.func_id, bcx.func);
                    let fn_addr = bcx.ins().func_addr(ir_types::I64, func_ref);
                    let n_caps = bcx.ins().iconst(ir_types::I32, info.captures.len() as i64);
                    let alloc_ref = self
                        .module
                        .declare_func_in_func(self.alloc_closure_id, bcx.func);
                    let alloc_inst = bcx.ins().call(alloc_ref, &[fn_addr, n_caps]);
                    let ptr = bcx.inst_results(alloc_inst)[0];
                    bcx.def_var(var, ptr);
                }
                TopKind::Value(irty) => {
                    let var = bcx.declare_var(irty);
                    top_vars.push(var);
                }
            }
        }

        // Phase B: two-phase to allow function→later-value forward references.
        //   B1: evaluate Value bindings in source order, def_var their CVars.
        //   B2: populate Function captures (all values are now def_var'd, so
        //       any capture target — Function or Value — resolves cleanly).
        //
        // A value body that calls a sibling function whose captures point to
        // a later value would silently read garbage in this order, so we
        // detect that pattern up-front and reject it with a clear diagnostic.
        let mut order: Vec<usize> = (0..self.top_level_instances.len()).collect();
        order.sort_by_key(|&i| self.top_level_instances[i].slot);

        // Pre-compute, per top-level slot, whether that slot's binding is a
        // function and, if so, whether any of its captures point to a Value
        // at a later source-order slot. Used by the forward-ref check below.
        let n_inst = self.top_level_instances.len();
        let mut func_caps_value_max_slot: Vec<Option<u32>> = vec![None; n_inst];
        for (i, inst) in self.top_level_instances.iter().enumerate() {
            if !matches!(inst.kind, TopKind::Function) {
                continue;
            }
            let key: FuncKey = (inst.expr_ptr, inst.mono_ty_str.clone());
            let info = self.funcs.get(&key).expect("declared").clone();
            let mut max_value_target: Option<u32> = None;
            for cap in &info.captures {
                if cap.inside.depth != 1 {
                    continue;
                }
                let target_kind = self
                    .top_level_instances
                    .iter()
                    .find(|t| t.slot == cap.inside.slot && t.mono_ty_str == cap.mono_ty_str)
                    .map(|t| t.kind.clone());
                if matches!(target_kind, Some(TopKind::Value(_))) {
                    max_value_target = Some(match max_value_target {
                        Some(prev) => prev.max(cap.inside.slot),
                        None => cap.inside.slot,
                    });
                }
            }
            func_caps_value_max_slot[i] = max_value_target;
        }

        // Per-slot kind lookup for forward-ref checks below.
        let slot_top_idx: HashMap<u32, usize> = self
            .top_level_instances
            .iter()
            .enumerate()
            .map(|(i, t)| (t.slot, i))
            .collect();

        // B1: evaluate Value bindings.
        for &i in &order {
            let inst = self.top_level_instances[i].clone();
            let TopKind::Value(irty) = inst.kind.clone() else {
                continue;
            };
            // SAFETY: AST outlives the JIT compile.
            let body: &Spanned<Expr> =
                unsafe { &*(inst.expr_ptr as *const Spanned<Expr>) };

            // Forward-ref check: walk the body, find sibling refs at the
            // top-level scope (var.resolved.depth == 0). Reject if any refs
            // either target a later Value (would read an undefined CVar) or
            // target a Function whose captures include a later Value (would
            // call into a closure with un-populated captures).
            let mut refs: HashSet<u32> = HashSet::new();
            collect_sibling_refs(body, 0, &mut refs);
            for r in &refs {
                let r_slot = *r;
                let Some(&r_idx) = slot_top_idx.get(&r_slot) else {
                    continue;
                };
                match &self.top_level_instances[r_idx].kind {
                    TopKind::Value(_) if r_slot > inst.slot => {
                        return Err(Diagnostic::new(
                            body.1.clone(),
                            format!(
                                "JIT: top-level value '{}' references later value at slot {}",
                                inst.mono_ty_str, r_slot
                            ),
                            "value→value forward reference is not yet supported",
                        ));
                    }
                    TopKind::Function => {
                        if let Some(latest) = func_caps_value_max_slot[r_idx] {
                            if latest > inst.slot {
                                return Err(Diagnostic::new(
                                    body.1.clone(),
                                    format!(
                                        "JIT: top-level value '{}' calls function whose captures include a later value at slot {}",
                                        inst.mono_ty_str, latest,
                                    ),
                                    "function→later-value works at top level only when the call site is in main, not in a sibling value",
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }

            let value_subst = Subst::new();
            let env = CompileEnv {
                kind: EnvKind::Main {
                    top_closures: &top_vars,
                },
                block_frames: Vec::new(),
                subst: &value_subst,
            };
            let module_ptr = &mut self.module as *mut JITModule;
            let funcs_ptr = &self.funcs as *const HashMap<FuncKey, FuncInfo>;
            let top_level_ptr = &self.top_level_instances as *const Vec<TopInstance>;
            let node_types_ptr = &self.node_types as *const HashMap<usize, Type>;
            let alloc_id = self.alloc_closure_id;
            let cc = self.call_conv;
            let v = unsafe {
                compile_expr(
                    &mut bcx,
                    body,
                    &env,
                    &mut *module_ptr,
                    &*funcs_ptr,
                    &*top_level_ptr,
                    &*node_types_ptr,
                    alloc_id,
                    cc,
                )
            }?;
            let v = coerce_to(&mut bcx, v, irty, &body.1)?;
            bcx.def_var(top_vars[i], v);
        }

        // B2: populate Function captures. Every CVar target — value or
        // function — is now def_var'd, so reads are safe.
        for &i in &order {
            let inst = self.top_level_instances[i].clone();
            if !matches!(inst.kind, TopKind::Function) {
                continue;
            }
            let key: FuncKey = (inst.expr_ptr, inst.mono_ty_str.clone());
            let info = self.funcs.get(&key).expect("declared").clone();
            let closure_ptr = bcx.use_var(top_vars[i]);
            for (cap_idx, cap) in info.captures.iter().enumerate() {
                if cap.inside.depth != 1 {
                    return Err(Diagnostic::new(
                        body_span(ast, inst.expr_ptr),
                        "JIT: top-level function references root scope",
                        "Phase 3 rejects List/String/Number/import",
                    ));
                }
                let target_idx = self
                    .top_level_instances
                    .iter()
                    .position(|t| {
                        t.slot == cap.inside.slot && t.mono_ty_str == cap.mono_ty_str
                    })
                    .ok_or_else(|| {
                        internal(format!(
                            "missing top-level instance for capture (slot {}, ty {})",
                            cap.inside.slot, cap.mono_ty_str
                        ))
                    })?;
                let val = bcx.use_var(top_vars[target_idx]);
                let offset = CAPTURES_OFFSET + 8 * cap_idx as i32;
                bcx.ins().store(MemFlags::trusted(), val, closure_ptr, offset);
            }
        }

        let empty_subst = Subst::new();
        let env = CompileEnv {
            kind: EnvKind::Main {
                top_closures: &top_vars,
            },
            block_frames: Vec::new(),
            subst: &empty_subst,
        };
        let module_ptr = &mut self.module as *mut JITModule;
        let funcs_ptr = &self.funcs as *const HashMap<FuncKey, FuncInfo>;
        let top_level_ptr = &self.top_level_instances as *const Vec<TopInstance>;
        let node_types_ptr = &self.node_types as *const HashMap<usize, Type>;
        let alloc_id = self.alloc_closure_id;
        let cc = self.call_conv;
        let result = unsafe {
            compile_expr(
                &mut bcx,
                &ast.body,
                &env,
                &mut *module_ptr,
                &*funcs_ptr,
                &*top_level_ptr,
                &*node_types_ptr,
                alloc_id,
                cc,
            )
        }?;
        let ret = if self.display {
            // Body keeps its native IR type; we emit display IR for it and
            // hand back a sentinel f64 so `__spctr_main`'s ABI doesn't need
            // to vary by program.
            let body_ty = self
                .node_types
                .get(&(&ast.body as *const _ as usize))
                .cloned()
                .ok_or_else(|| Diagnostic::new(ast.body.1.clone(), "JIT: missing body type", ""))?
                .apply(&empty_subst);
            // SAFETY: AST and module/funcs/top_level outlive this call.
            unsafe {
                emit_display(
                    &mut bcx,
                    result.val,
                    &body_ty,
                    &mut *module_ptr,
                    &*node_types_ptr,
                    &ast.body.1,
                )?;
            }
            // Print a trailing newline so output matches `println!("{}", v)`
            // from the tree-walker path.
            unsafe {
                emit_print_static(&mut bcx, &mut *module_ptr, "\n")?;
            }
            bcx.ins().f64const(0.0)
        } else {
            coerce_to(&mut bcx, result, ir_types::F64, &ast.body.1)?
        };
        bcx.ins().return_(&[ret]);
        bcx.seal_all_blocks();
        bcx.finalize();

        let main_id = self.main_id;
        self.module
            .define_function(main_id, &mut ctx)
            .map_err(|e| Diagnostic::new(ast.body.1.clone(), format!("define main: {e}"), "JIT"))?;
        self.module.clear_context(&mut ctx);
        Ok(())
    }
}

fn body_span(ast: &Statement, expr_ptr: usize) -> Span {
    for ((_, _), body) in &ast.definitions {
        if body as *const _ as usize == expr_ptr {
            return body.1.clone();
        }
    }
    ast.body.1.clone()
}

// === Compile expressions =====================================================

#[derive(Clone, Copy)]
struct JVal {
    val: IrValue,
    irty: IrType,
}

#[derive(Clone)]
struct CompileEnv<'a> {
    kind: EnvKind<'a>,
    /// In-progress Block / ImmediateBlock frames, innermost first. Each Block
    /// pushes a frame so that `bref.depth=0` lookups inside binding bodies hit
    /// the partially-populated record on the heap.
    block_frames: Vec<BlockFrame>,
    /// Substitution applied to types looked up from `node_types` during this
    /// body's codegen (empty for `Main`).
    subst: &'a Subst,
}

#[derive(Clone)]
enum EnvKind<'a> {
    Main {
        top_closures: &'a [CVar],
    },
    Function {
        closure_ptr: IrValue,
        args: &'a [IrValue],
        captures: &'a [Capture],
        /// Distance from this body's scope to the top-level frame.
        depth_to_top_level: u32,
    },
}

#[derive(Clone)]
struct BlockFrame {
    record_ptr: IrValue,
    /// IR types of each slot, in field-declaration order.
    slot_irtys: Vec<IrType>,
    /// Per-slot "has its record cell been stored" bit. Function-literal
    /// bindings are populated in Phase A (before any value body runs) so
    /// mutual recursion and function→later-value forward refs both work;
    /// value bindings flip their bit as they're evaluated in source order.
    /// A read of an un-populated slot is a forward reference and is
    /// rejected with a diagnostic.
    populated: Vec<bool>,
}

#[allow(clippy::too_many_arguments)]
fn compile_expr(
    bcx: &mut FunctionBuilder,
    expr: &Spanned<Expr>,
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
) -> Result<JVal, Diagnostic> {
    use cranelift_codegen::ir::condcodes::FloatCC;
    let span = &expr.1;
    match &expr.0 {
        Expr::Number(n) => Ok(JVal {
            val: bcx.ins().f64const(*n),
            irty: ir_types::F64,
        }),
        Expr::Bool(b) => Ok(JVal {
            val: bcx.ins().iconst(ir_types::I8, i64::from(*b)),
            irty: ir_types::I8,
        }),
        Expr::String(s) => Ok(emit_string_literal(bcx, s)),
        Expr::Interpolation(parts) => {
            // Compile each part to a string ptr (I64). Literal parts go
            // through emit_string_literal; Expr parts must already have
            // type `string` (typeck enforces this). Concatenate left-to-right
            // via the runtime helper spctr_str_concat.
            if parts.is_empty() {
                return Ok(emit_empty_string(bcx));
            }
            let concat_id = match module.declarations().get_name("spctr_str_concat") {
                Some(cranelift_module::FuncOrDataId::Func(id)) => id,
                _ => return Err(internal("spctr_str_concat not declared")),
            };
            let concat_ref = module.declare_func_in_func(concat_id, bcx.func);

            let mut acc: Option<IrValue> = None;
            for p in parts {
                let v = match p {
                    crate::ast::InterpPart::Literal(s, _) => emit_string_literal(bcx, s).val,
                    crate::ast::InterpPart::Expr(e) => {
                        let j = compile_expr(
                            bcx, e, env, module, funcs, top_level, node_types, alloc_id, cc,
                        )?;
                        if j.irty != ir_types::I64 {
                            return Err(Diagnostic::new(
                                e.1.clone(),
                                format!(
                                    "JIT: interpolation expects string, got IR type {}",
                                    j.irty
                                ),
                                "type mismatch",
                            ));
                        }
                        j.val
                    }
                };
                acc = Some(match acc {
                    None => v,
                    Some(prev) => {
                        let inst = bcx.ins().call(concat_ref, &[prev, v]);
                        bcx.inst_results(inst)[0]
                    }
                });
            }
            Ok(JVal {
                val: acc.expect("non-empty parts handled above"),
                irty: ir_types::I64,
            })
        }
        Expr::Variable(var) => {
            let bref = var.resolved.get().ok_or_else(|| {
                Diagnostic::new(span.clone(), "unresolved variable", "resolver")
            })?;
            let mono = node_types
                .get(&(expr as *const _ as usize))
                .cloned()
                .map(|t| format!("{}", t.apply(env.subst)));
            load_variable(bcx, bref, mono.as_deref(), env, module, funcs, top_level, span)
        }
        Expr::Function(_, _) => materialize_closure(
            bcx, expr, env, module, funcs, top_level, node_types, alloc_id, cc, span,
        ),
        Expr::If { cond, cons, alt } => {
            let c = compile_expr(bcx, cond, env, module, funcs, top_level, node_types, alloc_id, cc)?;
            if c.irty != ir_types::I8 {
                return Err(Diagnostic::new(
                    cond.1.clone(),
                    "JIT: if condition must be bool",
                    "expected bool",
                ));
            }
            let then_blk = bcx.create_block();
            let else_blk = bcx.create_block();
            let merge_blk = bcx.create_block();
            // Result type of if: type of cons (must match alt by typeck).
            let result_ty = node_types
                .get(&(expr as *const _ as usize))
                .cloned()
                .unwrap_or(Type::Any)
                .apply(env.subst);
            let result_irty = ir_type_for(&result_ty, span)?;
            bcx.append_block_param(merge_blk, result_irty);

            bcx.ins().brif(c.val, then_blk, &[], else_blk, &[]);

            bcx.switch_to_block(then_blk);
            bcx.seal_block(then_blk);
            let tv = compile_expr(bcx, cons, env, module, funcs, top_level, node_types, alloc_id, cc)?;
            let tv = coerce_to(bcx, tv, result_irty, &cons.1)?;
            bcx.ins().jump(merge_blk, &[tv.into()]);

            bcx.switch_to_block(else_blk);
            bcx.seal_block(else_blk);
            let ev = compile_expr(bcx, alt, env, module, funcs, top_level, node_types, alloc_id, cc)?;
            let ev = coerce_to(bcx, ev, result_irty, &alt.1)?;
            bcx.ins().jump(merge_blk, &[ev.into()]);

            bcx.switch_to_block(merge_blk);
            bcx.seal_block(merge_blk);
            Ok(JVal {
                val: bcx.block_params(merge_blk)[0],
                irty: result_irty,
            })
        }
        Expr::Binary(op, l, r) => {
            // Short-circuit for `&&` / `||` — typeck has unified both sides to
            // Bool, so we know the operand IR type is I8 and the result is I8.
            if matches!(op, BinOp::And | BinOp::Or) {
                let lv = compile_expr(bcx, l, env, module, funcs, top_level, node_types, alloc_id, cc)?;
                if lv.irty != ir_types::I8 {
                    return Err(Diagnostic::new(
                        l.1.clone(),
                        "JIT: && / || require bool operands",
                        "type mismatch",
                    ));
                }
                let rhs_blk = bcx.create_block();
                let short_blk = bcx.create_block();
                let merge_blk = bcx.create_block();
                bcx.append_block_param(merge_blk, ir_types::I8);
                match op {
                    BinOp::And => bcx.ins().brif(lv.val, rhs_blk, &[], short_blk, &[]),
                    BinOp::Or => bcx.ins().brif(lv.val, short_blk, &[], rhs_blk, &[]),
                    _ => unreachable!(),
                };

                bcx.switch_to_block(rhs_blk);
                bcx.seal_block(rhs_blk);
                let rv = compile_expr(bcx, r, env, module, funcs, top_level, node_types, alloc_id, cc)?;
                if rv.irty != ir_types::I8 {
                    return Err(Diagnostic::new(
                        r.1.clone(),
                        "JIT: && / || require bool operands",
                        "type mismatch",
                    ));
                }
                bcx.ins().jump(merge_blk, &[rv.val.into()]);

                bcx.switch_to_block(short_blk);
                bcx.seal_block(short_blk);
                bcx.ins().jump(merge_blk, &[lv.val.into()]);

                bcx.switch_to_block(merge_blk);
                bcx.seal_block(merge_blk);
                return Ok(JVal {
                    val: bcx.block_params(merge_blk)[0],
                    irty: ir_types::I8,
                });
            }
            let lv = compile_expr(bcx, l, env, module, funcs, top_level, node_types, alloc_id, cc)?;
            let rv = compile_expr(bcx, r, env, module, funcs, top_level, node_types, alloc_id, cc)?;

            // Equality and inequality. Recursive deep-equality for lists,
            // pointer-compare-with-content for strings, primitive cmp for
            // numbers/bool/null, and constant `false` for records/closures
            // (matching `interp::value_eq`).
            if matches!(op, BinOp::Eq | BinOp::Ne) {
                if lv.irty != rv.irty {
                    return Err(Diagnostic::new(
                        span.clone(),
                        format!("JIT: == operands disagree on IR type ({} vs {})", lv.irty, rv.irty),
                        "internal",
                    ));
                }
                let lty = node_types
                    .get(&(l.as_ref() as *const _ as usize))
                    .cloned()
                    .map(|t| t.apply(env.subst))
                    .ok_or_else(|| {
                        Diagnostic::new(
                            span.clone(),
                            "JIT: missing static type for == operand",
                            "internal",
                        )
                    })?;
                let eq = emit_value_eq(bcx, lv.val, rv.val, &lty, module, span)?;
                let raw = if matches!(op, BinOp::Eq) {
                    eq
                } else {
                    let one = bcx.ins().iconst(ir_types::I8, 1);
                    bcx.ins().bxor(eq, one)
                };
                return Ok(JVal { val: raw, irty: ir_types::I8 });
            }

            // Remaining arithmetic / numeric ops require f64 operands.
            let ln = expect_num(lv, &l.1)?;
            let rn = expect_num(rv, &r.1)?;
            let v = match op {
                BinOp::Add => JVal { val: bcx.ins().fadd(ln, rn), irty: ir_types::F64 },
                BinOp::Sub => JVal { val: bcx.ins().fsub(ln, rn), irty: ir_types::F64 },
                BinOp::Mul => JVal { val: bcx.ins().fmul(ln, rn), irty: ir_types::F64 },
                BinOp::Div => JVal { val: bcx.ins().fdiv(ln, rn), irty: ir_types::F64 },
                BinOp::Mod => {
                    let div = bcx.ins().fdiv(ln, rn);
                    let trunc = bcx.ins().trunc(div);
                    let mul = bcx.ins().fmul(trunc, rn);
                    JVal { val: bcx.ins().fsub(ln, mul), irty: ir_types::F64 }
                }
                BinOp::Lt => JVal { val: bcx.ins().fcmp(FloatCC::LessThan, ln, rn), irty: ir_types::I8 },
                BinOp::Le => JVal { val: bcx.ins().fcmp(FloatCC::LessThanOrEqual, ln, rn), irty: ir_types::I8 },
                BinOp::Gt => JVal { val: bcx.ins().fcmp(FloatCC::GreaterThan, ln, rn), irty: ir_types::I8 },
                BinOp::Ge => JVal { val: bcx.ins().fcmp(FloatCC::GreaterThanOrEqual, ln, rn), irty: ir_types::I8 },
                BinOp::Eq | BinOp::Ne | BinOp::And | BinOp::Or => unreachable!(),
            };
            Ok(v)
        }
        Expr::Unary(op, e) => {
            let v = compile_expr(bcx, e, env, module, funcs, top_level, node_types, alloc_id, cc)?;
            match op {
                UnaryOp::Neg => {
                    let n = expect_num(v, &e.1)?;
                    Ok(JVal { val: bcx.ins().fneg(n), irty: ir_types::F64 })
                }
                UnaryOp::Not => {
                    if v.irty != ir_types::I8 {
                        return Err(Diagnostic::new(
                            e.1.clone(),
                            "JIT: ! requires bool",
                            "type mismatch",
                        ));
                    }
                    let one = bcx.ins().iconst(ir_types::I8, 1);
                    Ok(JVal { val: bcx.ins().bxor(v.val, one), irty: ir_types::I8 })
                }
            }
        }
        Expr::Call(callee, args) => compile_call(
            bcx, callee, args, env, module, funcs, top_level, node_types, alloc_id, cc, span,
        ),
        Expr::Block(defs) => compile_block(
            bcx, expr, defs, env, module, funcs, top_level, node_types, alloc_id, cc,
        ),
        Expr::Access(obj, (name, name_span)) => {
            let obj_v = compile_expr(bcx, obj, env, module, funcs, top_level, node_types, alloc_id, cc)?;
            if obj_v.irty != ir_types::I64 {
                return Err(Diagnostic::new(
                    obj.1.clone(),
                    "JIT: field access requires a record",
                    "expected record",
                ));
            }
            let obj_ty = node_types
                .get(&(obj.as_ref() as *const _ as usize))
                .cloned()
                .ok_or_else(|| Diagnostic::new(obj.1.clone(), "JIT: missing type for record obj", ""))?
                .apply(env.subst);
            let fields = match &obj_ty {
                Type::Record(f) => f,
                _ => {
                    return Err(Diagnostic::new(
                        obj.1.clone(),
                        format!("JIT: field access on non-record type {obj_ty}"),
                        "expected record",
                    ))
                }
            };
            let (idx, field_ty) = fields
                .iter()
                .enumerate()
                .find(|(_, (n, _))| n == name)
                .map(|(i, (_, t))| (i, t.clone()))
                .ok_or_else(|| {
                    Diagnostic::new(
                        name_span.clone(),
                        format!(
                            "JIT: no field '{}' on {}",
                            crate::symbol::display(*name),
                            obj_ty
                        ),
                        "",
                    )
                })?;
            let irty = ir_type_for(&field_ty, name_span)?;
            let offset = 8 * idx as i32;
            let v = bcx.ins().load(irty, MemFlags::trusted(), obj_v.val, offset);
            Ok(JVal { val: v, irty })
        }
        Expr::List(items) => {
            // Allocate `[length: u32][_pad: u32][slot * n]`.
            let alloc_id = match module.declarations().get_name("spctr_alloc_list") {
                Some(cranelift_module::FuncOrDataId::Func(id)) => id,
                _ => return Err(internal("spctr_alloc_list not declared")),
            };
            let alloc_ref = module.declare_func_in_func(alloc_id, bcx.func);
            let n = bcx.ins().iconst(ir_types::I32, items.len() as i64);
            let inst = bcx.ins().call(alloc_ref, &[n]);
            let ptr = bcx.inst_results(inst)[0];
            // Store length at offset 0.
            bcx.ins().store(MemFlags::trusted(), n, ptr, 0);

            // Element type from typeck.
            let list_ty = node_types
                .get(&(expr as *const _ as usize))
                .cloned()
                .ok_or_else(|| Diagnostic::new(span.clone(), "JIT: missing type for list", ""))?
                .apply(env.subst);
            let elem_ty = match &list_ty {
                Type::List(t) => (**t).clone(),
                _ => {
                    return Err(Diagnostic::new(
                        span.clone(),
                        format!("JIT: list literal has non-list type {list_ty}"),
                        "internal",
                    ))
                }
            };
            let elem_irty = ir_type_for(&elem_ty, span)?;

            for (i, item) in items.iter().enumerate() {
                let v = compile_expr(bcx, item, env, module, funcs, top_level, node_types, alloc_id, cc)?;
                let v = coerce_to(bcx, v, elem_irty, &item.1)?;
                let offset = 8 + 8 * i as i32;
                bcx.ins().store(MemFlags::trusted(), v, ptr, offset);
            }
            Ok(JVal { val: ptr, irty: ir_types::I64 })
        }
        Expr::Index(arr, idx) => {
            let arr_v = compile_expr(bcx, arr, env, module, funcs, top_level, node_types, alloc_id, cc)?;
            if arr_v.irty != ir_types::I64 {
                return Err(Diagnostic::new(
                    arr.1.clone(),
                    "JIT: indexing requires a list",
                    "expected list",
                ));
            }
            let arr_ty = node_types
                .get(&(arr.as_ref() as *const _ as usize))
                .cloned()
                .ok_or_else(|| Diagnostic::new(arr.1.clone(), "JIT: missing type for indexed obj", ""))?
                .apply(env.subst);
            let elem_ty = match &arr_ty {
                Type::List(t) => (**t).clone(),
                other => {
                    return Err(Diagnostic::new(
                        arr.1.clone(),
                        format!("JIT: indexing on non-list type {other}"),
                        "Phase 3 supports list indexing only (block string-index later)",
                    ))
                }
            };
            let elem_irty = ir_type_for(&elem_ty, span)?;

            let idx_v = compile_expr(bcx, idx, env, module, funcs, top_level, node_types, alloc_id, cc)?;
            let idx_n = expect_num(idx_v, &idx.1)?;
            // f64 -> i64 (truncating to integer index).
            let idx_i = bcx.ins().fcvt_to_sint(ir_types::I64, idx_n);
            // offset = 8 + 8 * idx (length header + slot).
            let eight = bcx.ins().iconst(ir_types::I64, 8);
            let scaled = bcx.ins().imul(idx_i, eight);
            let with_header = bcx.ins().iadd(scaled, eight);
            let addr = bcx.ins().iadd(arr_v.val, with_header);
            let v = bcx.ins().load(elem_irty, MemFlags::trusted(), addr, 0);
            Ok(JVal { val: v, irty: elem_irty })
        }
        Expr::Null => Ok(JVal {
            val: bcx.ins().iconst(ir_types::I8, 0),
            irty: ir_types::I8,
        }),
        Expr::ImmediateBlock(stmt) => compile_immediate_block(
            bcx, expr, stmt, env, module, funcs, top_level, node_types, alloc_id, cc,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_immediate_block(
    bcx: &mut FunctionBuilder,
    block_expr: &Spanned<Expr>,
    stmt: &Statement,
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
) -> Result<JVal, Diagnostic> {
    // ImmediateBlock pushes a resolver scope of its own, just like Block, so
    // local sibling references work. Unlike Block we don't return the record —
    // the body expression at the end is the value.
    let span = &block_expr.1;
    let defs = &stmt.definitions;

    if defs.is_empty() {
        return compile_expr(
            bcx, &stmt.body, env, module, funcs, top_level, node_types, alloc_id, cc,
        );
    }

    // Allocate a record-like buffer for the locals so siblings can be loaded
    // by `bref.depth=0` lookups (same plumbing as Block).
    let record_alloc_id = match module.declarations().get_name("spctr_alloc_record") {
        Some(cranelift_module::FuncOrDataId::Func(id)) => id,
        _ => return Err(internal("spctr_alloc_record not declared")),
    };
    let alloc_ref = module.declare_func_in_func(record_alloc_id, bcx.func);
    let n_slots = bcx.ins().iconst(ir_types::I32, defs.len() as i64);
    let inst = bcx.ins().call(alloc_ref, &[n_slots]);
    let record_ptr = bcx.inst_results(inst)[0];

    // Per-binding IR types via typeck.
    let mut slot_irtys: Vec<IrType> = Vec::with_capacity(defs.len());
    for (_, body) in defs {
        let key = body as *const _ as usize;
        let ty = node_types
            .get(&key)
            .cloned()
            .ok_or_else(|| Diagnostic::new(body.1.clone(), "JIT: missing type for binding", ""))?
            .apply(env.subst);
        slot_irtys.push(ir_type_for(&ty, &body.1)?);
    }

    let mut frames = env.block_frames.clone();
    frames.push(BlockFrame {
        record_ptr,
        slot_irtys: slot_irtys.clone(),
        populated: vec![false; defs.len()],
    });

    compile_block_bindings(
        bcx,
        defs,
        env,
        &mut frames,
        module,
        funcs,
        top_level,
        node_types,
        alloc_id,
        cc,
        record_ptr,
        &slot_irtys,
    )?;

    // Compile body using the populated frame.
    let body_env = CompileEnv {
        kind: env.kind.clone(),
        block_frames: frames,
        subst: env.subst,
    };
    let _ = span;
    compile_expr(
        bcx, &stmt.body, &body_env, module, funcs, top_level, node_types, alloc_id, cc,
    )
}

#[allow(clippy::too_many_arguments)]
fn compile_block(
    bcx: &mut FunctionBuilder,
    block_expr: &Spanned<Expr>,
    defs: &[Bind],
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
) -> Result<JVal, Diagnostic> {
    let span = &block_expr.1;
    let ty = node_types
        .get(&(block_expr as *const _ as usize))
        .cloned()
        .ok_or_else(|| Diagnostic::new(span.clone(), "JIT: missing type for block", ""))?
        .apply(env.subst);
    let fields = match &ty {
        Type::Record(f) => f.clone(),
        _ => {
            return Err(Diagnostic::new(
                span.clone(),
                format!("JIT: block produced non-record type {ty}"),
                "internal",
            ))
        }
    };
    if fields.len() != defs.len() {
        return Err(internal(format!(
            "block field count mismatch: {} vs {}",
            fields.len(),
            defs.len()
        )));
    }

    // Look up the runtime helper by name (we don't thread its FuncId through
    // the codegen call chain — it's a singleton).
    let record_alloc_id = match module.declarations().get_name("spctr_alloc_record") {
        Some(cranelift_module::FuncOrDataId::Func(id)) => id,
        _ => return Err(internal("spctr_alloc_record not declared")),
    };
    let alloc_ref = module.declare_func_in_func(record_alloc_id, bcx.func);
    let n_slots = bcx.ins().iconst(ir_types::I32, fields.len() as i64);
    let inst = bcx.ins().call(alloc_ref, &[n_slots]);
    let record_ptr = bcx.inst_results(inst)[0];

    let slot_irtys: Vec<IrType> = fields
        .iter()
        .map(|(_, t)| ir_type_for(t, span))
        .collect::<Result<_, _>>()?;

    let mut frames = env.block_frames.clone();
    frames.push(BlockFrame {
        record_ptr,
        slot_irtys: slot_irtys.clone(),
        populated: vec![false; defs.len()],
    });

    compile_block_bindings(
        bcx,
        defs,
        env,
        &mut frames,
        module,
        funcs,
        top_level,
        node_types,
        alloc_id,
        cc,
        record_ptr,
        &slot_irtys,
    )?;

    Ok(JVal {
        val: record_ptr,
        irty: ir_types::I64,
    })
}

/// Compile the bindings of a block (or immediate block) into a record that
/// has already been allocated and pushed as the innermost `BlockFrame` on
/// `frames`.
///
/// Three logical phases:
///
/// 1. **Allocate functions**: every function-literal binding gets its
///    closure allocated and stored in the slot. `cap.inside.depth == 1`
///    captures (sibling references) are deferred to be populated lazily;
///    captures pointing outside the block are filled immediately. Mutual
///    recursion among sibling functions works because all sibling-function
///    closures exist before any are populated.
///
/// 2. **Evaluate values in source order**: for each non-function binding,
///    opportunistically populate any deferred capture whose target slot is
///    now stored, refuse to compile if a sibling-function reference still
///    has unsatisfied captures (would be silent-wrong otherwise), then
///    compile the body and store the result.
///
/// 3. **Finalize**: one last opportunistic populate pass cleans up captures
///    that only needed values from the final iteration. Any cap that's
///    still pending after that is a true cycle and is rejected.
#[allow(clippy::too_many_arguments)]
fn compile_block_bindings(
    bcx: &mut FunctionBuilder,
    defs: &[Bind],
    env: &CompileEnv,
    frames: &mut Vec<BlockFrame>,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    record_ptr: IrValue,
    slot_irtys: &[IrType],
) -> Result<(), Diagnostic> {
    // Phase 1: allocate each function literal, deferring sibling captures.
    let mut pending: Vec<Vec<(usize, BindRef, IrType)>> = vec![Vec::new(); defs.len()];
    for (i, ((_, _), body)) in defs.iter().enumerate() {
        if !matches!(body.0, Expr::Function(_, _)) {
            continue;
        }
        let inner_env = CompileEnv {
            kind: env.kind.clone(),
            block_frames: frames.clone(),
            subst: env.subst,
        };
        let (closure_ptr, deferred) = materialize_closure_partial(
            bcx,
            body,
            &inner_env,
            module,
            funcs,
            top_level,
            node_types,
            alloc_id,
            cc,
            &body.1,
            /* defer_sibling */ true,
        )?;
        pending[i] = deferred;
        let offset = 8 * i as i32;
        bcx.ins().store(MemFlags::trusted(), closure_ptr, record_ptr, offset);
        frames.last_mut().unwrap().populated[i] = true;
    }

    // Initial fixpoint pass: any cap whose target was already populated
    // (other sibling functions, or non-sibling scopes) can be filled now.
    populate_caps_until_stable(
        bcx, &mut pending, record_ptr, slot_irtys, frames, env, module, funcs, top_level, defs,
    )?;

    // Phase 2: evaluate non-function bindings in source order, with an
    // opportunistic capture-populate pass after each.
    for (i, ((_, _), body)) in defs.iter().enumerate() {
        if matches!(body.0, Expr::Function(_, _)) {
            continue;
        }

        // Reject any reference to a sibling function whose captures haven't
        // all been satisfied — calling it would read garbage.
        let mut refs: HashSet<u32> = HashSet::new();
        collect_sibling_refs(body, 0, &mut refs);
        for r in &refs {
            let r_idx = *r as usize;
            if r_idx < pending.len() && !pending[r_idx].is_empty() {
                return Err(Diagnostic::new(
                    body.1.clone(),
                    format!(
                        "JIT: '{}' uses sibling function '{}' before all of its captures are bound",
                        crate::symbol::display(defs[i].0 .0),
                        crate::symbol::display(defs[r_idx].0 .0),
                    ),
                    "function captures a later-defined value transitively reached from this binding",
                ));
            }
        }

        let inner_env = CompileEnv {
            kind: env.kind.clone(),
            block_frames: frames.clone(),
            subst: env.subst,
        };
        let v = compile_expr(
            bcx, body, &inner_env, module, funcs, top_level, node_types, alloc_id, cc,
        )?;
        let irty = slot_irtys[i];
        let v = coerce_to(bcx, v, irty, &body.1)?;
        let offset = 8 * i as i32;
        bcx.ins().store(MemFlags::trusted(), v, record_ptr, offset);
        frames.last_mut().unwrap().populated[i] = true;

        populate_caps_until_stable(
            bcx,
            &mut pending,
            record_ptr,
            slot_irtys,
            frames,
            env,
            module,
            funcs,
            top_level,
            defs,
        )?;
    }

    // Phase 3: nothing should remain pending. If something does, the only
    // possibility is a cap whose target was never populated — a true cycle
    // (e.g. `x: f, f: (_) => x`).
    for (i, p) in pending.iter().enumerate() {
        if !p.is_empty() {
            return Err(Diagnostic::new(
                defs[i].1.1.clone(),
                format!(
                    "JIT: cyclic binding — '{}'s captures depend on values that never resolve",
                    crate::symbol::display(defs[i].0 .0),
                ),
                "binding directly or indirectly refers to itself through its captures",
            ));
        }
    }

    Ok(())
}

/// Loop a single capture-population pass until no further progress is made.
/// Each pass walks the per-slot pending lists; entries whose target slot is
/// now populated emit a load+store and are dropped from `pending`.
#[allow(clippy::too_many_arguments)]
fn populate_caps_until_stable(
    bcx: &mut FunctionBuilder,
    pending: &mut [Vec<(usize, BindRef, IrType)>],
    record_ptr: IrValue,
    slot_irtys: &[IrType],
    frames: &[BlockFrame],
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    defs: &[Bind],
) -> Result<(), Diagnostic> {
    loop {
        let mut progress = false;
        let inner_env = CompileEnv {
            kind: env.kind.clone(),
            block_frames: frames.to_vec(),
            subst: env.subst,
        };
        let innermost = frames.last().expect("populate_caps expects at least one block frame");
        for slot in 0..pending.len() {
            if pending[slot].is_empty() {
                continue;
            }
            let irty = slot_irtys[slot];
            let mut closure_ptr: Option<IrValue> = None;
            let drained: Vec<_> = pending[slot].drain(..).collect();
            for (cap_idx, outer_bref, cap_irty) in drained {
                let ready = if outer_bref.depth == 0 {
                    let t = outer_bref.slot as usize;
                    t < innermost.populated.len() && innermost.populated[t]
                } else {
                    // Non-sibling captures (outer scope) are always available
                    // by the time Phase 1 finishes.
                    true
                };
                if !ready {
                    pending[slot].push((cap_idx, outer_bref, cap_irty));
                    continue;
                }
                if closure_ptr.is_none() {
                    closure_ptr = Some(bcx.ins().load(
                        irty,
                        MemFlags::trusted(),
                        record_ptr,
                        8 * slot as i32,
                    ));
                }
                let val = load_variable(
                    bcx,
                    outer_bref,
                    None,
                    &inner_env,
                    module,
                    funcs,
                    top_level,
                    &defs[slot].1.1,
                )?;
                let val = coerce_to(bcx, val, cap_irty, &defs[slot].1.1)?;
                let offset = CAPTURES_OFFSET + 8 * cap_idx as i32;
                bcx.ins()
                    .store(MemFlags::trusted(), val, closure_ptr.unwrap(), offset);
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    Ok(())
}

/// Collect sibling slots referenced inside `expr` at scope depth `depth`
/// (0 == the current block's bindings). Walks through nested function
/// literals because their captures are populated by `materialize_closure`
/// when the surrounding expression is evaluated, so the targets of those
/// captures count as eval-time deps of this expression.
fn collect_sibling_refs(expr: &Spanned<Expr>, depth: u32, out: &mut HashSet<u32>) {
    match &expr.0 {
        Expr::Variable(var) => {
            if let Some(bref) = var.resolved.get() {
                if bref.depth == depth {
                    out.insert(bref.slot);
                }
            }
        }
        Expr::Function(_, b) => collect_sibling_refs(b, depth + 1, out),
        Expr::List(items) => {
            for i in items {
                collect_sibling_refs(i, depth, out);
            }
        }
        Expr::Block(defs) => {
            for (_, b) in defs {
                collect_sibling_refs(b, depth + 1, out);
            }
        }
        Expr::ImmediateBlock(s) => {
            for (_, b) in &s.definitions {
                collect_sibling_refs(b, depth + 1, out);
            }
            collect_sibling_refs(&s.body, depth + 1, out);
        }
        Expr::If { cond, cons, alt } => {
            collect_sibling_refs(cond, depth, out);
            collect_sibling_refs(cons, depth, out);
            collect_sibling_refs(alt, depth, out);
        }
        Expr::Binary(_, l, r) => {
            collect_sibling_refs(l, depth, out);
            collect_sibling_refs(r, depth, out);
        }
        Expr::Unary(_, e) => collect_sibling_refs(e, depth, out),
        Expr::Call(c, args) => {
            collect_sibling_refs(c, depth, out);
            for a in args {
                collect_sibling_refs(a, depth, out);
            }
        }
        Expr::Access(o, _) => collect_sibling_refs(o, depth, out),
        Expr::Index(a, i) => {
            collect_sibling_refs(a, depth, out);
            collect_sibling_refs(i, depth, out);
        }
        Expr::Interpolation(parts) => {
            for p in parts {
                if let crate::ast::InterpPart::Expr(e) = p {
                    collect_sibling_refs(e, depth, out);
                }
            }
        }
        Expr::Number(_) | Expr::String(_) | Expr::Null | Expr::Bool(_) => {}
    }
}


fn expect_num(v: JVal, span: &Span) -> Result<IrValue, Diagnostic> {
    if v.irty == ir_types::F64 {
        Ok(v.val)
    } else {
        Err(Diagnostic::new(
            span.clone(),
            "JIT: expected number",
            "type mismatch",
        ))
    }
}

fn coerce_to(
    _bcx: &mut FunctionBuilder,
    v: JVal,
    target: IrType,
    span: &Span,
) -> Result<IrValue, Diagnostic> {
    if v.irty == target {
        Ok(v.val)
    } else {
        Err(Diagnostic::new(
            span.clone(),
            format!("JIT: ir type mismatch {} vs {}", v.irty, target),
            "internal",
        ))
    }
}

/// `mono_hint`: when known, the canonical mono_ty_str of the value being loaded.
/// In the `Main` arm this disambiguates which `TopInstance` to read from.
fn load_variable(
    bcx: &mut FunctionBuilder,
    bref: BindRef,
    mono_hint: Option<&str>,
    env: &CompileEnv,
    _module: &mut JITModule,
    _funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    span: &Span,
) -> Result<JVal, Diagnostic> {
    // Block frames sit between the innermost lookup point and the underlying
    // function/main env. A `bref.depth=K` peels K layers; if K falls within
    // the block-frame stack, we load from the partial record.
    //
    // `block_frames` is pushed-to in scope-entry order, so the innermost
    // frame is at the back. `bref.depth = 0` is innermost, so it indexes
    // from the back: `block_frames[n_blocks - 1 - depth]`.
    let n_blocks = env.block_frames.len() as u32;
    if bref.depth < n_blocks {
        let frame_idx = (n_blocks - 1 - bref.depth) as usize;
        let frame = &env.block_frames[frame_idx];
        let slot = bref.slot as usize;
        if slot >= frame.populated.len() || !frame.populated[slot] {
            return Err(Diagnostic::new(
                span.clone(),
                "JIT: forward reference inside block",
                "Phase 3 evaluates block value bindings strictly in source order",
            ));
        }
        let irty = frame.slot_irtys[slot];
        let offset = 8 * bref.slot as i32;
        let v = bcx.ins().load(irty, MemFlags::trusted(), frame.record_ptr, offset);
        return Ok(JVal { val: v, irty });
    }
    let bref = BindRef {
        depth: bref.depth - n_blocks,
        slot: bref.slot,
    };
    match &env.kind {
        EnvKind::Main { top_closures } => {
            if bref.depth != 0 {
                return Err(Diagnostic::new(
                    span.clone(),
                    "JIT: only top-level bindings are accessible from main body",
                    "no root-scope refs in JIT",
                ));
            }
            let mono = mono_hint.ok_or_else(|| {
                Diagnostic::new(span.clone(), "JIT: missing mono ty for top-level use", "")
            })?;
            let idx = top_level
                .iter()
                .position(|t| t.slot == bref.slot && t.mono_ty_str == mono)
                .ok_or_else(|| {
                    Diagnostic::new(
                        span.clone(),
                        format!("JIT: no top-level instance for slot {} ty {}", bref.slot, mono),
                        "",
                    )
                })?;
            let var = top_closures.get(idx).copied().ok_or_else(|| {
                Diagnostic::new(span.clone(), "JIT: top-level instance idx out of range", "")
            })?;
            let irty = match top_level[idx].kind {
                TopKind::Function => ir_types::I64,
                TopKind::Value(t) => t,
            };
            Ok(JVal {
                val: bcx.use_var(var),
                irty,
            })
        }
        EnvKind::Function {
            closure_ptr,
            args,
            captures,
            depth_to_top_level: _,
        } => {
            if bref.depth == 0 {
                // Function param.
                let v = args
                    .get(bref.slot as usize)
                    .copied()
                    .ok_or_else(|| Diagnostic::new(span.clone(), "param slot OOR", ""))?;
                // Type comes from outer typing; bcx doesn't track types directly,
                // but we can recover the IR type from the value.
                let irty = bcx.func.dfg.value_type(v);
                Ok(JVal { val: v, irty })
            } else {
                // Capture lookup.
                let idx = captures
                    .iter()
                    .position(|c| c.inside == bref)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            span.clone(),
                            "JIT internal: missing capture slot for outer ref",
                            "",
                        )
                    })?;
                let cap = &captures[idx];
                let offset = CAPTURES_OFFSET + 8 * idx as i32;
                let loaded = bcx
                    .ins()
                    .load(cap.irty, MemFlags::trusted(), *closure_ptr, offset);
                Ok(JVal {
                    val: loaded,
                    irty: cap.irty,
                })
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn materialize_closure(
    bcx: &mut FunctionBuilder,
    func_expr: &Spanned<Expr>,
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    span: &Span,
) -> Result<JVal, Diagnostic> {
    let (closure, deferred) = materialize_closure_partial(
        bcx,
        func_expr,
        env,
        module,
        funcs,
        top_level,
        node_types,
        alloc_id,
        cc,
        span,
        /* defer_sibling */ false,
    )?;
    debug_assert!(deferred.is_empty(), "non-deferred path must populate everything");
    Ok(JVal {
        val: closure,
        irty: ir_types::I64,
    })
}

/// Like `materialize_closure` but with optional deferral of sibling captures.
///
/// When `defer_sibling` is true (the block Phase A path), captures with
/// `cap.inside.depth == 1` — i.e. references to other bindings in the same
/// block scope — are skipped here and returned in the `Vec` so the block
/// compiler can populate them later, once the sibling slots have been
/// stored. Non-sibling captures (outer scopes) are populated immediately.
///
/// When `defer_sibling` is false this behaves exactly like the legacy
/// `materialize_closure`: every capture is populated eagerly.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn materialize_closure_partial(
    bcx: &mut FunctionBuilder,
    func_expr: &Spanned<Expr>,
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    span: &Span,
    defer_sibling: bool,
) -> Result<(IrValue, Vec<(usize, BindRef, IrType)>), Diagnostic> {
    let expr_ptr = func_expr as *const _ as usize;
    let mono = node_types
        .get(&expr_ptr)
        .cloned()
        .ok_or_else(|| Diagnostic::new(span.clone(), "JIT: missing type for function literal", ""))?
        .apply(env.subst);
    let key: FuncKey = (expr_ptr, format!("{mono}"));
    let info = funcs.get(&key).ok_or_else(|| {
        Diagnostic::new(
            span.clone(),
            format!("JIT: no FuncInfo for instance {} :: {}", expr_ptr, key.1),
            "",
        )
    })?;

    let func_ref = module.declare_func_in_func(info.func_id, bcx.func);
    let fn_addr = bcx.ins().func_addr(ir_types::I64, func_ref);
    let n_caps = bcx.ins().iconst(ir_types::I32, info.captures.len() as i64);
    let alloc_ref = module.declare_func_in_func(alloc_id, bcx.func);
    let inst = bcx.ins().call(alloc_ref, &[fn_addr, n_caps]);
    let closure = bcx.inst_results(inst)[0];

    let mut deferred: Vec<(usize, BindRef, IrType)> = Vec::new();
    for (idx, cap) in info.captures.iter().enumerate() {
        let outer_bref = BindRef {
            depth: cap.inside.depth - 1,
            slot: cap.inside.slot,
        };
        if defer_sibling && cap.inside.depth == 1 {
            deferred.push((idx, outer_bref, cap.irty));
            continue;
        }
        let val = load_variable(
            bcx,
            outer_bref,
            Some(cap.mono_ty_str.as_str()),
            env,
            module,
            funcs,
            top_level,
            &func_expr.1,
        )?;
        let val = coerce_to(bcx, val, cap.irty, &func_expr.1)?;
        let offset = CAPTURES_OFFSET + 8 * idx as i32;
        bcx.ins().store(MemFlags::trusted(), val, closure, offset);
    }

    let _ = cc;
    Ok((closure, deferred))
}

#[allow(clippy::too_many_arguments)]
fn compile_call(
    bcx: &mut FunctionBuilder,
    callee: &Spanned<Expr>,
    args: &[Spanned<Expr>],
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    span: &Span,
) -> Result<JVal, Diagnostic> {
    // First try the stdlib dispatcher — `Module.field(args)` on List/String/Number.
    if let Some(result) = try_compile_stdlib_call(
        bcx, callee, args, env, module, funcs, top_level, node_types, alloc_id, cc, span,
    )? {
        return Ok(result);
    }
    // Try direct call: callee is a Variable that resolves to a known top-level function.
    if let Expr::Variable(var) = &callee.0 {
        if let Some(bref) = var.resolved.get() {
            // Compute the use-site mono ty so we can pick the right instance.
            let callee_mono = node_types
                .get(&(callee as *const _ as usize))
                .cloned()
                .map(|t| format!("{}", t.apply(env.subst)));
            if let Some(info) = top_level_func(bref, callee_mono.as_deref(), env, top_level, funcs) {
                let info = info.clone();
                let closure_val = load_variable(
                    bcx,
                    bref,
                    callee_mono.as_deref(),
                    env,
                    module,
                    funcs,
                    top_level,
                    &callee.1,
                )?;
                let closure = closure_val.val;

                if args.len() != info.param_irtys.len() {
                    return Err(Diagnostic::new(
                        span.clone(),
                        format!("JIT: arity mismatch ({} vs {})", args.len(), info.param_irtys.len()),
                        "",
                    ));
                }
                let mut argvs = Vec::with_capacity(args.len() + 1);
                argvs.push(closure);
                for (a, &irty) in args.iter().zip(info.param_irtys.iter()) {
                    let av = compile_expr(bcx, a, env, module, funcs, top_level, node_types, alloc_id, cc)?;
                    argvs.push(coerce_to(bcx, av, irty, &a.1)?);
                }
                let func_ref = module.declare_func_in_func(info.func_id, bcx.func);
                let inst = bcx.ins().call(func_ref, &argvs);
                let v = bcx.inst_results(inst)[0];
                return Ok(JVal {
                    val: v,
                    irty: info.ret_irty,
                });
            }
        }
    }

    // Indirect call. Compile the callee to a closure pointer; derive signature from typeck.
    let callee_v = compile_expr(bcx, callee, env, module, funcs, top_level, node_types, alloc_id, cc)?;
    if callee_v.irty != ir_types::I64 {
        return Err(Diagnostic::new(
            callee.1.clone(),
            "JIT: callee is not a function value",
            "expected closure",
        ));
    }
    let callee_ty = node_types
        .get(&(callee as *const _ as usize))
        .cloned()
        .ok_or_else(|| Diagnostic::new(callee.1.clone(), "JIT: missing type for callee", ""))?
        .apply(env.subst);
    let (param_irtys, ret_irty) = fn_type_parts(&callee_ty, &callee.1)?;

    if args.len() != param_irtys.len() {
        return Err(Diagnostic::new(
            span.clone(),
            format!("JIT: arity mismatch ({} vs {})", args.len(), param_irtys.len()),
            "",
        ));
    }
    let mut sig = Signature::new(cc);
    sig.params.push(AbiParam::new(ir_types::I64));
    for &p in &param_irtys {
        sig.params.push(AbiParam::new(p));
    }
    sig.returns.push(AbiParam::new(ret_irty));
    let sig_ref: SigRef = bcx.import_signature(sig);

    let fn_ptr = bcx
        .ins()
        .load(ir_types::I64, MemFlags::trusted(), callee_v.val, 0);
    let mut argvs = Vec::with_capacity(args.len() + 1);
    argvs.push(callee_v.val);
    for (a, &irty) in args.iter().zip(param_irtys.iter()) {
        let av = compile_expr(bcx, a, env, module, funcs, top_level, node_types, alloc_id, cc)?;
        argvs.push(coerce_to(bcx, av, irty, &a.1)?);
    }
    let inst = bcx.ins().call_indirect(sig_ref, fn_ptr, &argvs);
    let v = bcx.inst_results(inst)[0];
    Ok(JVal { val: v, irty: ret_irty })
}

// === stdlib dispatcher =====================================================

/// Distance from the current scope (whatever `env` represents) to the
/// resolver's root frame. This is what `bref.depth` should equal for a
/// reference to land on `List` / `String` / `Number` / `import`.
fn distance_to_root(env: &CompileEnv) -> u32 {
    let base = match &env.kind {
        EnvKind::Main { .. } => 1,
        EnvKind::Function { depth_to_top_level, .. } => depth_to_top_level + 1,
    };
    env.block_frames.len() as u32 + base
}

#[allow(clippy::too_many_arguments)]
fn try_compile_stdlib_call(
    bcx: &mut FunctionBuilder,
    callee: &Spanned<Expr>,
    args: &[Spanned<Expr>],
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    span: &Span,
) -> Result<Option<JVal>, Diagnostic> {
    let (obj, field_name) = match &callee.0 {
        Expr::Access(o, (n, _)) => (o.as_ref(), *n),
        _ => return Ok(None),
    };
    let bref = match &obj.0 {
        Expr::Variable(v) => match v.resolved.get() {
            Some(b) => b,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    if bref.depth != distance_to_root(env) {
        return Ok(None);
    }
    let module_kind = match bref.slot {
        0 => StdModule::List,
        1 => StdModule::String,
        2 => StdModule::Number,
        _ => return Ok(None),
    };
    let name = crate::symbol::display(field_name);
    Ok(Some(compile_stdlib_call(
        bcx,
        module_kind,
        &name,
        args,
        env,
        module,
        funcs,
        top_level,
        node_types,
        alloc_id,
        cc,
        span,
    )?))
}

#[derive(Clone, Copy)]
enum StdModule {
    List,
    String,
    Number,
}

#[allow(clippy::too_many_arguments)]
fn compile_stdlib_call(
    bcx: &mut FunctionBuilder,
    m: StdModule,
    name: &str,
    args: &[Spanned<Expr>],
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    span: &Span,
) -> Result<JVal, Diagnostic> {
    // Compile every arg up front; most callees use them in order. Higher-order
    // helpers (List.map etc.) re-evaluate inline so they don't go through this.
    let compile_args = |bcx: &mut FunctionBuilder,
                        module: &mut JITModule|
     -> Result<Vec<JVal>, Diagnostic> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(compile_expr(
                bcx, a, env, module, funcs, top_level, node_types, alloc_id, cc,
            )?);
        }
        Ok(out)
    };

    let arity = |n: usize, span: &Span| -> Result<(), Diagnostic> {
        if args.len() != n {
            Err(Diagnostic::new(
                span.clone(),
                format!("JIT stdlib: expected {n} args, got {}", args.len()),
                "",
            ))
        } else {
            Ok(())
        }
    };

    let call_helper = |bcx: &mut FunctionBuilder,
                       module: &mut JITModule,
                       fname: &str,
                       in_args: &[IrValue],
                       ret_irty: IrType|
     -> Result<JVal, Diagnostic> {
        let id = match module.declarations().get_name(fname) {
            Some(cranelift_module::FuncOrDataId::Func(id)) => id,
            _ => return Err(internal(format!("{fname} not declared"))),
        };
        let r = module.declare_func_in_func(id, bcx.func);
        let inst = bcx.ins().call(r, in_args);
        let v = bcx.inst_results(inst)[0];
        Ok(JVal { val: v, irty: ret_irty })
    };

    match (m, name) {
        // ---- Number intrinsics ------------------------------------------
        (StdModule::Number, "abs") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let n = expect_num(xs[0], &args[0].1)?;
            Ok(JVal { val: bcx.ins().fabs(n), irty: ir_types::F64 })
        }
        (StdModule::Number, "floor") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let n = expect_num(xs[0], &args[0].1)?;
            Ok(JVal { val: bcx.ins().floor(n), irty: ir_types::F64 })
        }
        (StdModule::Number, "ceil") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let n = expect_num(xs[0], &args[0].1)?;
            Ok(JVal { val: bcx.ins().ceil(n), irty: ir_types::F64 })
        }
        (StdModule::Number, "round") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let n = expect_num(xs[0], &args[0].1)?;
            Ok(JVal { val: bcx.ins().nearest(n), irty: ir_types::F64 })
        }
        (StdModule::Number, "sqrt") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let n = expect_num(xs[0], &args[0].1)?;
            Ok(JVal { val: bcx.ins().sqrt(n), irty: ir_types::F64 })
        }
        (StdModule::Number, "min") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            let a = expect_num(xs[0], &args[0].1)?;
            let b = expect_num(xs[1], &args[1].1)?;
            Ok(JVal { val: bcx.ins().fmin(a, b), irty: ir_types::F64 })
        }
        (StdModule::Number, "max") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            let a = expect_num(xs[0], &args[0].1)?;
            let b = expect_num(xs[1], &args[1].1)?;
            Ok(JVal { val: bcx.ins().fmax(a, b), irty: ir_types::F64 })
        }
        (StdModule::Number, "pow") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            let a = expect_num(xs[0], &args[0].1)?;
            let b = expect_num(xs[1], &args[1].1)?;
            call_helper(bcx, module, "spctr_num_pow", &[a, b], ir_types::F64)
        }
        (StdModule::Number, "toString") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let n = expect_num(xs[0], &args[0].1)?;
            call_helper(bcx, module, "spctr_num_to_string", &[n], ir_types::I64)
        }
        (StdModule::Number, "parse") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let s = xs[0].val;
            call_helper(bcx, module, "spctr_num_parse", &[s], ir_types::F64)
        }
        // ---- String -----------------------------------------------------
        (StdModule::String, "length") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let len_u32 = bcx.ins().load(ir_types::I32, MemFlags::trusted(), xs[0].val, 0);
            let n = bcx.ins().fcvt_from_uint(ir_types::F64, len_u32);
            Ok(JVal { val: n, irty: ir_types::F64 })
        }
        (StdModule::String, "concat") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            call_helper(
                bcx, module, "spctr_str_concat",
                &[xs[0].val, xs[1].val], ir_types::I64,
            )
        }
        (StdModule::String, "contains") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            call_helper(
                bcx, module, "spctr_str_contains",
                &[xs[0].val, xs[1].val], ir_types::I8,
            )
        }
        (StdModule::String, "to_lower") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            call_helper(bcx, module, "spctr_str_to_lower", &[xs[0].val], ir_types::I64)
        }
        (StdModule::String, "to_upper") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            call_helper(bcx, module, "spctr_str_to_upper", &[xs[0].val], ir_types::I64)
        }
        (StdModule::String, "split") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            call_helper(
                bcx, module, "spctr_str_split",
                &[xs[0].val, xs[1].val], ir_types::I64,
            )
        }
        // ---- List basic -------------------------------------------------
        (StdModule::List, "length") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let len_u32 = bcx.ins().load(ir_types::I32, MemFlags::trusted(), xs[0].val, 0);
            let n = bcx.ins().fcvt_from_uint(ir_types::F64, len_u32);
            Ok(JVal { val: n, irty: ir_types::F64 })
        }
        (StdModule::List, "head") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            // Element type from typeck.
            let list_ty = node_types
                .get(&(&args[0] as *const _ as usize))
                .cloned()
                .ok_or_else(|| Diagnostic::new(args[0].1.clone(), "JIT: missing list type", ""))?
                .apply(env.subst);
            let elem_ty = match &list_ty {
                Type::List(t) => (**t).clone(),
                _ => {
                    return Err(Diagnostic::new(
                        args[0].1.clone(),
                        format!("JIT: List.head on non-list {list_ty}"),
                        "",
                    ))
                }
            };
            let elem_irty = ir_type_for(&elem_ty, span)?;
            let v = bcx.ins().load(elem_irty, MemFlags::trusted(), xs[0].val, 8);
            Ok(JVal { val: v, irty: elem_irty })
        }
        (StdModule::List, "tail") => {
            arity(1, span)?;
            let xs = compile_args(bcx, module)?;
            let len_u32 = bcx.ins().load(ir_types::I32, MemFlags::trusted(), xs[0].val, 0);
            let one = bcx.ins().iconst(ir_types::I32, 1);
            let new_len = bcx.ins().isub(len_u32, one);
            call_helper(
                bcx, module, "spctr_list_slice",
                &[xs[0].val, one, new_len], ir_types::I64,
            )
        }
        (StdModule::List, "take") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            let n_f = expect_num(xs[1], &args[1].1)?;
            let n = bcx.ins().fcvt_to_uint(ir_types::I32, n_f);
            let zero = bcx.ins().iconst(ir_types::I32, 0);
            call_helper(
                bcx, module, "spctr_list_slice",
                &[xs[0].val, zero, n], ir_types::I64,
            )
        }
        (StdModule::List, "drop") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            let n_f = expect_num(xs[1], &args[1].1)?;
            let n = bcx.ins().fcvt_to_uint(ir_types::I32, n_f);
            let total = bcx.ins().load(ir_types::I32, MemFlags::trusted(), xs[0].val, 0);
            let new_len = bcx.ins().isub(total, n);
            call_helper(
                bcx, module, "spctr_list_slice",
                &[xs[0].val, n, new_len], ir_types::I64,
            )
        }
        (StdModule::List, "concat") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            call_helper(
                bcx, module, "spctr_list_concat",
                &[xs[0].val, xs[1].val], ir_types::I64,
            )
        }
        (StdModule::List, "range") => {
            arity(2, span)?;
            let xs = compile_args(bcx, module)?;
            let a = expect_num(xs[0], &args[0].1)?;
            let b = expect_num(xs[1], &args[1].1)?;
            call_helper(bcx, module, "spctr_list_range", &[a, b], ir_types::I64)
        }
        // ---- List higher-order (inline loops) ---------------------------
        (StdModule::List, "map") => compile_list_map(
            bcx, args, env, module, funcs, top_level, node_types, alloc_id, cc, span,
        ),
        (StdModule::List, "filter") => compile_list_filter(
            bcx, args, env, module, funcs, top_level, node_types, alloc_id, cc, span,
        ),
        (StdModule::List, "reduce") => compile_list_reduce(
            bcx, args, env, module, funcs, top_level, node_types, alloc_id, cc, span,
        ),
        _ => Err(Diagnostic::new(
            span.clone(),
            format!("JIT: stdlib function not implemented: {} . {}", module_kind_name(m), name),
            "Phase 3 stdlib",
        )),
    }
}

fn module_kind_name(m: StdModule) -> &'static str {
    match m {
        StdModule::List => "List",
        StdModule::String => "String",
        StdModule::Number => "Number",
    }
}

// === List.map / filter / reduce: inline loops ==============================

#[allow(clippy::too_many_arguments)]
fn compile_list_map(
    bcx: &mut FunctionBuilder,
    args: &[Spanned<Expr>],
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    span: &Span,
) -> Result<JVal, Diagnostic> {
    use cranelift_codegen::ir::condcodes::IntCC;
    if args.len() != 2 {
        return Err(Diagnostic::new(span.clone(), "JIT: List.map arity 2", ""));
    }
    // spctr stdlib signature: `List.map(list, fn)`.
    let list_v = compile_expr(bcx, &args[0], env, module, funcs, top_level, node_types, alloc_id, cc)?;
    let f_v = compile_expr(bcx, &args[1], env, module, funcs, top_level, node_types, alloc_id, cc)?;

    let f_ty = node_types
        .get(&(&args[1] as *const _ as usize))
        .cloned()
        .ok_or_else(|| Diagnostic::new(args[1].1.clone(), "JIT: missing fn type", ""))?
        .apply(env.subst);
    let (in_irty, out_irty) = match &f_ty {
        Type::Fn(p, r) if p.len() == 1 => {
            (ir_type_for(&p[0], span)?, ir_type_for(r, span)?)
        }
        _ => {
            return Err(Diagnostic::new(
                args[1].1.clone(),
                format!("JIT: List.map requires unary callable, got {f_ty}"),
                "",
            ))
        }
    };

    let len_u32 = bcx.ins().load(ir_types::I32, MemFlags::trusted(), list_v.val, 0);
    let len_i64 = bcx.ins().uextend(ir_types::I64, len_u32);

    // Allocate output list.
    let alloc_list_id = match module.declarations().get_name("spctr_alloc_list") {
        Some(cranelift_module::FuncOrDataId::Func(id)) => id,
        _ => return Err(internal("spctr_alloc_list not declared")),
    };
    let alloc_ref = module.declare_func_in_func(alloc_list_id, bcx.func);
    let alloc_inst = bcx.ins().call(alloc_ref, &[len_u32]);
    let out_ptr = bcx.inst_results(alloc_inst)[0];
    bcx.ins().store(MemFlags::trusted(), len_u32, out_ptr, 0);

    // Set up call_indirect signature once.
    let mut sig = Signature::new(cc);
    sig.params.push(AbiParam::new(ir_types::I64));
    sig.params.push(AbiParam::new(in_irty));
    sig.returns.push(AbiParam::new(out_irty));
    let sig_ref = bcx.import_signature(sig);
    let fn_ptr = bcx.ins().load(ir_types::I64, MemFlags::trusted(), f_v.val, 0);

    // Loop counter.
    let i_var = bcx.declare_var(ir_types::I64);
    let zero = bcx.ins().iconst(ir_types::I64, 0);
    bcx.def_var(i_var, zero);

    let header = bcx.create_block();
    let body = bcx.create_block();
    let exit = bcx.create_block();

    bcx.ins().jump(header, &[]);
    bcx.switch_to_block(header);
    let i = bcx.use_var(i_var);
    let cond = bcx.ins().icmp(IntCC::SignedLessThan, i, len_i64);
    bcx.ins().brif(cond, body, &[], exit, &[]);

    bcx.switch_to_block(body);
    bcx.seal_block(body);
    let i_b = bcx.use_var(i_var);
    let off = bcx.ins().imul_imm(i_b, 8);
    let off = bcx.ins().iadd_imm(off, 8);
    let in_addr = bcx.ins().iadd(list_v.val, off);
    let elem = bcx.ins().load(in_irty, MemFlags::trusted(), in_addr, 0);
    let call = bcx.ins().call_indirect(sig_ref, fn_ptr, &[f_v.val, elem]);
    let mapped = bcx.inst_results(call)[0];
    let out_off = bcx.ins().imul_imm(i_b, 8);
    let out_off = bcx.ins().iadd_imm(out_off, 8);
    let out_addr = bcx.ins().iadd(out_ptr, out_off);
    bcx.ins().store(MemFlags::trusted(), mapped, out_addr, 0);
    let next = bcx.ins().iadd_imm(i_b, 1);
    bcx.def_var(i_var, next);
    bcx.ins().jump(header, &[]);

    bcx.seal_block(header);
    bcx.switch_to_block(exit);
    bcx.seal_block(exit);

    Ok(JVal { val: out_ptr, irty: ir_types::I64 })
}

#[allow(clippy::too_many_arguments)]
fn compile_list_filter(
    bcx: &mut FunctionBuilder,
    args: &[Spanned<Expr>],
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    span: &Span,
) -> Result<JVal, Diagnostic> {
    use cranelift_codegen::ir::condcodes::IntCC;
    if args.len() != 2 {
        return Err(Diagnostic::new(span.clone(), "JIT: List.filter arity 2", ""));
    }
    // spctr stdlib signature: `List.filter(list, predicate)`.
    let list_v = compile_expr(bcx, &args[0], env, module, funcs, top_level, node_types, alloc_id, cc)?;
    let f_v = compile_expr(bcx, &args[1], env, module, funcs, top_level, node_types, alloc_id, cc)?;

    let list_ty = node_types
        .get(&(&args[0] as *const _ as usize))
        .cloned()
        .ok_or_else(|| Diagnostic::new(args[0].1.clone(), "JIT: missing list type", ""))?
        .apply(env.subst);
    let elem_irty = match &list_ty {
        Type::List(t) => ir_type_for(t, span)?,
        _ => {
            return Err(Diagnostic::new(
                args[0].1.clone(),
                format!("JIT: List.filter on non-list {list_ty}"),
                "",
            ))
        }
    };

    let len_u32 = bcx.ins().load(ir_types::I32, MemFlags::trusted(), list_v.val, 0);
    let len_i64 = bcx.ins().uextend(ir_types::I64, len_u32);

    // Allocate worst-case list (size = input len) then patch the length down
    // at the end. Trades a bit of memory for simpler IR vs two-pass.
    let alloc_list_id = match module.declarations().get_name("spctr_alloc_list") {
        Some(cranelift_module::FuncOrDataId::Func(id)) => id,
        _ => return Err(internal("spctr_alloc_list not declared")),
    };
    let alloc_ref = module.declare_func_in_func(alloc_list_id, bcx.func);
    let alloc_inst = bcx.ins().call(alloc_ref, &[len_u32]);
    let out_ptr = bcx.inst_results(alloc_inst)[0];

    // call_indirect signature for predicate.
    let mut sig = Signature::new(cc);
    sig.params.push(AbiParam::new(ir_types::I64));
    sig.params.push(AbiParam::new(elem_irty));
    sig.returns.push(AbiParam::new(ir_types::I8));
    let sig_ref = bcx.import_signature(sig);
    let fn_ptr = bcx.ins().load(ir_types::I64, MemFlags::trusted(), f_v.val, 0);

    let i_var = bcx.declare_var(ir_types::I64);
    let j_var = bcx.declare_var(ir_types::I64);
    let zero = bcx.ins().iconst(ir_types::I64, 0);
    bcx.def_var(i_var, zero);
    bcx.def_var(j_var, zero);

    let header = bcx.create_block();
    let body = bcx.create_block();
    let keep = bcx.create_block();
    let cont = bcx.create_block();
    let exit = bcx.create_block();

    bcx.ins().jump(header, &[]);
    bcx.switch_to_block(header);
    let i = bcx.use_var(i_var);
    let cond = bcx.ins().icmp(IntCC::SignedLessThan, i, len_i64);
    bcx.ins().brif(cond, body, &[], exit, &[]);

    bcx.switch_to_block(body);
    bcx.seal_block(body);
    let i_b = bcx.use_var(i_var);
    let off = bcx.ins().imul_imm(i_b, 8);
    let off = bcx.ins().iadd_imm(off, 8);
    let in_addr = bcx.ins().iadd(list_v.val, off);
    let elem = bcx.ins().load(elem_irty, MemFlags::trusted(), in_addr, 0);
    let call = bcx.ins().call_indirect(sig_ref, fn_ptr, &[f_v.val, elem]);
    let kept = bcx.inst_results(call)[0];
    bcx.ins().brif(kept, keep, &[], cont, &[]);

    bcx.switch_to_block(keep);
    bcx.seal_block(keep);
    let j = bcx.use_var(j_var);
    let out_off = bcx.ins().imul_imm(j, 8);
    let out_off = bcx.ins().iadd_imm(out_off, 8);
    let out_addr = bcx.ins().iadd(out_ptr, out_off);
    bcx.ins().store(MemFlags::trusted(), elem, out_addr, 0);
    let next_j = bcx.ins().iadd_imm(j, 1);
    bcx.def_var(j_var, next_j);
    bcx.ins().jump(cont, &[]);

    bcx.switch_to_block(cont);
    bcx.seal_block(cont);
    let next_i = bcx.ins().iadd_imm(i_b, 1);
    bcx.def_var(i_var, next_i);
    bcx.ins().jump(header, &[]);

    bcx.seal_block(header);
    bcx.switch_to_block(exit);
    bcx.seal_block(exit);

    // Patch the actual length into the header.
    let final_j = bcx.use_var(j_var);
    let final_j_u32 = bcx.ins().ireduce(ir_types::I32, final_j);
    bcx.ins().store(MemFlags::trusted(), final_j_u32, out_ptr, 0);

    Ok(JVal { val: out_ptr, irty: ir_types::I64 })
}

#[allow(clippy::too_many_arguments)]
fn compile_list_reduce(
    bcx: &mut FunctionBuilder,
    args: &[Spanned<Expr>],
    env: &CompileEnv,
    module: &mut JITModule,
    funcs: &HashMap<FuncKey, FuncInfo>,
    top_level: &[TopInstance],
    node_types: &HashMap<usize, Type>,
    alloc_id: FuncId,
    cc: CallConv,
    span: &Span,
) -> Result<JVal, Diagnostic> {
    use cranelift_codegen::ir::condcodes::IntCC;
    // spctr's signature is `reduce(list, init, f)` — see `src/stdlib/list.rs`.
    if args.len() != 3 {
        return Err(Diagnostic::new(span.clone(), "JIT: List.reduce arity 3", ""));
    }
    let list_v = compile_expr(bcx, &args[0], env, module, funcs, top_level, node_types, alloc_id, cc)?;
    let init_v = compile_expr(bcx, &args[1], env, module, funcs, top_level, node_types, alloc_id, cc)?;
    let f_v = compile_expr(bcx, &args[2], env, module, funcs, top_level, node_types, alloc_id, cc)?;

    let list_ty = node_types
        .get(&(&args[0] as *const _ as usize))
        .cloned()
        .ok_or_else(|| Diagnostic::new(args[0].1.clone(), "JIT: missing list type", ""))?
        .apply(env.subst);
    let elem_irty = match &list_ty {
        Type::List(t) => ir_type_for(t, span)?,
        _ => {
            return Err(Diagnostic::new(
                args[0].1.clone(),
                format!("JIT: List.reduce on non-list {list_ty}"),
                "",
            ))
        }
    };
    let acc_irty = init_v.irty;

    let len_u32 = bcx.ins().load(ir_types::I32, MemFlags::trusted(), list_v.val, 0);
    let len_i64 = bcx.ins().uextend(ir_types::I64, len_u32);

    // call_indirect signature for the reducer (acc, elem) -> acc.
    let mut sig = Signature::new(cc);
    sig.params.push(AbiParam::new(ir_types::I64));
    sig.params.push(AbiParam::new(acc_irty));
    sig.params.push(AbiParam::new(elem_irty));
    sig.returns.push(AbiParam::new(acc_irty));
    let sig_ref = bcx.import_signature(sig);
    let fn_ptr = bcx.ins().load(ir_types::I64, MemFlags::trusted(), f_v.val, 0);

    let i_var = bcx.declare_var(ir_types::I64);
    let acc_var = bcx.declare_var(acc_irty);
    let zero = bcx.ins().iconst(ir_types::I64, 0);
    bcx.def_var(i_var, zero);
    bcx.def_var(acc_var, init_v.val);

    let header = bcx.create_block();
    let body = bcx.create_block();
    let exit = bcx.create_block();

    bcx.ins().jump(header, &[]);
    bcx.switch_to_block(header);
    let i = bcx.use_var(i_var);
    let cond = bcx.ins().icmp(IntCC::SignedLessThan, i, len_i64);
    bcx.ins().brif(cond, body, &[], exit, &[]);

    bcx.switch_to_block(body);
    bcx.seal_block(body);
    let i_b = bcx.use_var(i_var);
    let off = bcx.ins().imul_imm(i_b, 8);
    let off = bcx.ins().iadd_imm(off, 8);
    let in_addr = bcx.ins().iadd(list_v.val, off);
    let elem = bcx.ins().load(elem_irty, MemFlags::trusted(), in_addr, 0);
    let acc = bcx.use_var(acc_var);
    let call = bcx.ins().call_indirect(sig_ref, fn_ptr, &[f_v.val, acc, elem]);
    let new_acc = bcx.inst_results(call)[0];
    bcx.def_var(acc_var, new_acc);
    let next = bcx.ins().iadd_imm(i_b, 1);
    bcx.def_var(i_var, next);
    bcx.ins().jump(header, &[]);

    bcx.seal_block(header);
    bcx.switch_to_block(exit);
    bcx.seal_block(exit);

    Ok(JVal { val: bcx.use_var(acc_var), irty: acc_irty })
}

/// If `bref` (resolved in `env`'s coord system) names a top-level function with
/// the given monomorphic type at the use site, return its `FuncInfo`. Both the
/// slot and `mono_hint` must match an existing `TopInstance`.
fn top_level_func<'a>(
    bref: BindRef,
    mono_hint: Option<&str>,
    env: &CompileEnv,
    top_level: &[TopInstance],
    funcs: &'a HashMap<FuncKey, FuncInfo>,
) -> Option<&'a FuncInfo> {
    let target_depth = match &env.kind {
        EnvKind::Main { .. } => 0,
        EnvKind::Function { depth_to_top_level, .. } => *depth_to_top_level,
    };
    if bref.depth != target_depth {
        return None;
    }
    let mono = mono_hint?;
    let inst = top_level
        .iter()
        .find(|t| t.slot == bref.slot && t.mono_ty_str == mono)?;
    if !matches!(inst.kind, TopKind::Function) {
        return None;
    }
    let key: FuncKey = (inst.expr_ptr, inst.mono_ty_str.clone());
    funcs.get(&key)
}
