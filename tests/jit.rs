//! JIT smoke tests. Phase 1 only covers numeric programs; non-numeric inputs
//! must produce a Diagnostic without panicking.
use spctr::{interp, jit, parser, resolver};

fn jit_run(src: &str) -> Result<f64, String> {
    let ast = parser::parse(src).map_err(|ds| {
        ds.iter()
            .map(|d| format!("{}: {}", d.message, d.label))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    resolver::resolve(&ast, &interp::ROOT_NAMES)
        .map_err(|d| format!("{}: {}", d.message, d.label))?;
    jit::run(&ast).map_err(|d| format!("{}: {}", d.message, d.label))
}

#[test]
fn fib_matches_interp() {
    assert_eq!(
        jit_run("fib: (n) => if n < 2 then n else fib(n - 2) + fib(n - 1), fib(20)").unwrap(),
        6765.0,
    );
}

#[test]
fn arithmetic() {
    assert_eq!(jit_run("1 + 2 * 3").unwrap(), 7.0);
    assert_eq!(jit_run("(1 + 2) * 3").unwrap(), 9.0);
    assert_eq!(jit_run("10 % 3").unwrap(), 1.0);
    assert_eq!(jit_run("-5 + 3").unwrap(), -2.0);
}

#[test]
fn nested_calls() {
    let src = "
        sq: (n) => n * n,
        sum_sq: (a, b) => sq(a) + sq(b),
        sum_sq(3, 4)
    ";
    assert_eq!(jit_run(src).unwrap(), 25.0);
}

#[test]
fn string_equality() {
    assert_eq!(
        jit_run(r#"if "hello" == "hello" then 1 else 0"#).unwrap(),
        1.0
    );
    assert_eq!(
        jit_run(r#"if "hello" == "world" then 1 else 0"#).unwrap(),
        0.0
    );
}

#[test]
fn string_param_dispatch() {
    let src = r#"
        greet: (name) => if name == "world" then 100 else 0,
        greet("world") + greet("there")
    "#;
    assert_eq!(jit_run(src).unwrap(), 100.0);
}

#[test]
fn string_in_record_and_list() {
    // `&&` isn't supported yet, so we nest two `if`s.
    let src = r#"
        labeled: {label: "foo", value: 42},
        words: ["a", "b", "c"],
        if labeled.label == "foo" then
          (if words[1] == "b" then labeled.value else 0)
        else 0
    "#;
    assert_eq!(jit_run(src).unwrap(), 42.0);
}

#[test]
fn bool_equality() {
    assert_eq!(jit_run("if true == true then 1 else 0").unwrap(), 1.0);
    assert_eq!(jit_run("if true == false then 1 else 0").unwrap(), 0.0);
}

#[test]
fn stdlib_number_intrinsics() {
    assert_eq!(jit_run("Number.abs(-3)").unwrap(), 3.0);
    assert_eq!(jit_run("Number.floor(2.7)").unwrap(), 2.0);
    assert_eq!(jit_run("Number.ceil(2.3)").unwrap(), 3.0);
    assert_eq!(jit_run("Number.sqrt(Number.pow(3, 2) + Number.pow(4, 2))").unwrap(), 5.0);
    assert_eq!(jit_run("Number.min(7, 3)").unwrap(), 3.0);
    assert_eq!(jit_run("Number.max(7, 3)").unwrap(), 7.0);
}

#[test]
fn stdlib_string_basic() {
    assert_eq!(jit_run(r#"String.length("hello")"#).unwrap(), 5.0);
    assert_eq!(
        jit_run(r#"String.length(String.concat("foo", "bar"))"#).unwrap(),
        6.0
    );
    assert_eq!(
        jit_run(r#"if String.contains("hello world", "world") then 1 else 0"#).unwrap(),
        1.0,
    );
}

#[test]
fn stdlib_list_basic() {
    assert_eq!(jit_run("List.length([10, 20, 30, 40])").unwrap(), 4.0);
    assert_eq!(jit_run("List.length(List.range(0, 7))").unwrap(), 7.0);
    assert_eq!(jit_run("List.head([42, 0, 0])").unwrap(), 42.0);
    assert_eq!(jit_run("List.length(List.tail([1, 2, 3]))").unwrap(), 2.0);
    assert_eq!(jit_run("List.length(List.take([1, 2, 3, 4], 2))").unwrap(), 2.0);
    assert_eq!(jit_run("List.head(List.drop([10, 20, 30], 1))").unwrap(), 20.0);
    assert_eq!(
        jit_run("List.length(List.concat([1, 2], [3, 4, 5]))").unwrap(),
        5.0,
    );
}

#[test]
fn iterative_fib_with_record_accumulator() {
    // Combines lists, records, closures, captures, and reduce into one
    // program — a realistic stdlib-heavy spctr workload.
    let src = "
        fib_iter: (n) => List.reduce(
          List.range(0, n),
          {a: 0, b: 1},
          (s, _) => {a: s.b, b: s.a + s.b}
        ).a,
        fib_iter(40)
    ";
    assert_eq!(jit_run(src).unwrap(), 102334155.0);
}

#[test]
fn string_reduce() {
    let src = r#"
        words: ["hello", "world", "foo"],
        joined: List.reduce(words, "", (acc, w) => String.concat(acc, w)),
        String.length(joined)
    "#;
    assert_eq!(jit_run(src).unwrap(), 13.0);
}

#[test]
fn short_circuit_and() {
    assert_eq!(jit_run("if true && true then 1 else 0").unwrap(), 1.0);
    assert_eq!(jit_run("if true && false then 1 else 0").unwrap(), 0.0);
    assert_eq!(jit_run("if false && true then 1 else 0").unwrap(), 0.0);
}

#[test]
fn short_circuit_or() {
    assert_eq!(jit_run("if false || false then 1 else 0").unwrap(), 0.0);
    assert_eq!(jit_run("if false || true then 1 else 0").unwrap(), 1.0);
    assert_eq!(jit_run("if true || false then 1 else 0").unwrap(), 1.0);
}

#[test]
fn null_equality() {
    assert_eq!(jit_run("if null == null then 1 else 0").unwrap(), 1.0);
}

#[test]
fn immediate_block() {
    assert_eq!(jit_run("{a: 1, b: 2, a + b}").unwrap(), 3.0);
    assert_eq!(jit_run("{x: 10, y: x + 5, x * y}").unwrap(), 150.0);
}

#[test]
fn sum_of_squares_via_stdlib() {
    let src = "
        List.reduce(
          List.map(List.range(1, 101), (x) => x * x),
          0,
          (acc, x) => acc + x
        )
    ";
    assert_eq!(jit_run(src).unwrap(), 338350.0);
}

#[test]
fn stdlib_list_higher_order() {
    // map: square each element
    assert_eq!(
        jit_run(
            "
            xs: List.map([1, 2, 3, 4, 5], (x) => x * x),
            List.reduce(xs, 0, (acc, x) => acc + x)
            "
        )
        .unwrap(),
        55.0,
    );
    // filter: keep elements > 2
    assert_eq!(
        jit_run("List.length(List.filter([1, 2, 3, 4, 5], (x) => x > 2))").unwrap(),
        3.0,
    );
    // filter + reduce: sum 6..10
    assert_eq!(
        jit_run(
            "List.reduce(
               List.filter(List.range(1, 11), (x) => x > 5),
               0,
               (acc, x) => acc + x
             )"
        )
        .unwrap(),
        40.0,
    );
}

#[test]
fn list_literal_and_index() {
    let src = "
        xs: [10, 20, 30],
        xs[0] + xs[1] + xs[2]
    ";
    assert_eq!(jit_run(src).unwrap(), 60.0);
}

#[test]
fn list_polymorphic_access() {
    // `nth` is polymorphic over element type; here used at Number.
    let src = "
        nth: (l, n) => l[n],
        nth([5, 6, 7], 1)
    ";
    assert_eq!(jit_run(src).unwrap(), 6.0);
}

#[test]
fn list_of_records() {
    let src = "
        pts: [{x: 1, y: 2}, {x: 3, y: 4}],
        pts[0].x + pts[1].y
    ";
    assert_eq!(jit_run(src).unwrap(), 5.0);
}

#[test]
fn closures_with_capture() {
    let src = "make_adder: (x) => (y) => x + y, make_adder(3)(7)";
    assert_eq!(jit_run(src).unwrap(), 10.0);
}

#[test]
fn higher_order_twice() {
    let src = "twice: (f, x) => f(f(x)), inc: (x) => x + 1, twice(inc, 5)";
    assert_eq!(jit_run(src).unwrap(), 7.0);
}

#[test]
fn compose() {
    let src = "
        compose: (f, g, x) => f(g(x)),
        sq: (x) => x * x,
        inc: (x) => x + 1,
        compose(sq, inc, 3)
    ";
    assert_eq!(jit_run(src).unwrap(), 16.0);
}

#[test]
fn closure_chain() {
    // Closure returns closure returns number
    let src = "
        curry_add: (x) => (y) => (z) => x + y + z,
        curry_add(1)(10)(100)
    ";
    assert_eq!(jit_run(src).unwrap(), 111.0);
}

#[test]
fn record_construction_and_access() {
    let src = "
        make_pair: (a, b) => {first: a, second: b},
        sum: (p) => p.first + p.second,
        sum(make_pair(3, 4))
    ";
    assert_eq!(jit_run(src).unwrap(), 7.0);
}

#[test]
fn record_sibling_reference() {
    let src = "
        foo: (a) => {x: a, y: x + 1},
        foo(5).y
    ";
    assert_eq!(jit_run(src).unwrap(), 6.0);
}

#[test]
fn record_holding_closures() {
    // Records can hold closures and we can call them via field access.
    let src = "
        mk: (n) => {add: (x) => x + n, mul: (x) => x * n},
        mk(3).add(4) + mk(3).mul(5)
    ";
    assert_eq!(jit_run(src).unwrap(), 22.0);
}

#[test]
fn top_level_value_binding() {
    let src = "
        make_adder: (x) => (y) => x + y,
        add5: make_adder(5),
        add5(10)
    ";
    assert_eq!(jit_run(src).unwrap(), 15.0);
}

#[test]
fn function_captures_top_level_value() {
    let src = "
        n: 10,
        add_n: (x) => x + n,
        add_n(5)
    ";
    assert_eq!(jit_run(src).unwrap(), 15.0);
}

#[test]
fn top_level_record_binding() {
    let src = "
        mk_pt: (a, b) => {x: a, y: b},
        origin: mk_pt(0, 0),
        unit: mk_pt(1, 0),
        origin.x + unit.x
    ";
    assert_eq!(jit_run(src).unwrap(), 1.0);
}

#[test]
fn forward_value_reference_rejected() {
    let err = jit_run(
        "
        add_n: (x) => x + n,
        n: 10,
        add_n(5)
        ",
    )
    .unwrap_err();
    assert!(
        err.contains("source-order init") || err.contains("defined later"),
        "unexpected error: {err}"
    );
}

#[test]
fn record_forward_reference_rejected() {
    let err = jit_run(
        "
        foo: (a) => {y: x + 1, x: a},
        foo(5).y
        ",
    )
    .unwrap_err();
    assert!(err.contains("forward reference"), "unexpected error: {err}");
}

#[test]
fn polymorphic_multi_instance() {
    // `id` is used at two different monomorphic types in the same program.
    // Phase 2.5 monomorphizes via worklist BFS so each (slot, mono_ty) gets
    // its own compiled FuncId.
    let src = "
        id: (x) => x,
        inc: (x) => x + 1,
        apply_via_id: (f, x) => id(f)(x),
        id(5) + apply_via_id(inc, 7)
    ";
    assert_eq!(jit_run(src).unwrap(), 13.0);
}

#[test]
fn list_equality_number() {
    assert_eq!(
        jit_run("if [1, 2, 3] == [1, 2, 3] then 1 else 0").unwrap(),
        1.0
    );
    assert_eq!(
        jit_run("if [1, 2, 3] == [1, 2, 4] then 1 else 0").unwrap(),
        0.0
    );
    assert_eq!(
        jit_run("if [1, 2, 3] == [1, 2] then 1 else 0").unwrap(),
        0.0
    );
    assert_eq!(jit_run("if [1, 2] != [1, 3] then 1 else 0").unwrap(), 1.0);
}

#[test]
fn list_equality_string() {
    assert_eq!(
        jit_run(r#"if ["a", "b"] == ["a", "b"] then 1 else 0"#).unwrap(),
        1.0
    );
    assert_eq!(
        jit_run(r#"if ["a", "b"] == ["a", "c"] then 1 else 0"#).unwrap(),
        0.0
    );
}

#[test]
fn nested_list_equality() {
    assert_eq!(
        jit_run("if [[1, 2], [3]] == [[1, 2], [3]] then 1 else 0").unwrap(),
        1.0
    );
    assert_eq!(
        jit_run("if [[1, 2], [3]] == [[1, 2], [4]] then 1 else 0").unwrap(),
        0.0
    );
}

#[test]
fn record_equality_always_false() {
    // Matches `interp::value_eq` which has no Record arm and falls through
    // to `_ => false`.
    assert_eq!(
        jit_run("if {x: 1} == {x: 1} then 1 else 0").unwrap(),
        0.0
    );
    assert_eq!(
        jit_run("if {x: 1} != {x: 1} then 1 else 0").unwrap(),
        1.0
    );
}

#[test]
fn record_bracket_string_index() {
    // `r["x"]` is desugared by the parser to `r.x`, so it just needs to
    // type-check and lower the same way `.x` does.
    let src = r#"
        r: {x: 10, y: 20},
        r["x"] + r["y"]
    "#;
    assert_eq!(jit_run(src).unwrap(), 30.0);
}
