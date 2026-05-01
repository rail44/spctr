use insta::assert_snapshot;
use spctr::{interp, parser, resolver};

fn run(src: &str) -> String {
    let ast = match parser::parse(src) {
        Ok(a) => a,
        Err(diags) => {
            return format!(
                "[parse error]\n{}",
                diags
                    .iter()
                    .map(|d| format!("{}: {}", d.message, d.label))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    };
    if let Err(d) = resolver::resolve(&ast, &interp::ROOT_NAMES) {
        return format!("[resolve error] {}: {}", d.message, d.label);
    }
    match interp::run(&ast) {
        Ok(v) => v.to_string(),
        Err(d) => format!("[runtime error] {}: {}", d.message, d.label),
    }
}

#[test]
fn arithmetic() {
    assert_snapshot!(run("1 + 2 * 3"), @"7");
    assert_snapshot!(run("(1 + 2) * 3"), @"9");
    assert_snapshot!(run("10 % 3"), @"1");
    assert_snapshot!(run("-5 + 3"), @"-2");
}

#[test]
fn comparison() {
    assert_snapshot!(run("1 = 1"), @"true");
    assert_snapshot!(run("1 != 2"), @"true");
    assert_snapshot!(run("3 > 2"), @"true");
    assert_snapshot!(run("3 <= 3"), @"true");
}

#[test]
fn if_else() {
    assert_snapshot!(run("if 1 < 2 \"yes\" else \"no\""), @r###""yes""###);
    assert_snapshot!(run("if 1 > 2 \"yes\" else \"no\""), @r###""no""###);
}

#[test]
fn binds_and_blocks() {
    assert_snapshot!(run("x: 5, x + 10"), @"15");
    assert_snapshot!(run("x: {a: 1, b: 2}, x.a + x.b"), @"3");
    assert_snapshot!(run("x: {a: 1}, x[\"a\"]"), @"1");
}

#[test]
fn list() {
    assert_snapshot!(run("[1, 2, 3][1]"), @"2");
    assert_snapshot!(run("List.concat([1, 2], [3, 4])"), @"[1, 2, 3, 4]");
}

#[test]
fn closure() {
    assert_snapshot!(
        run("make: (n) => () => n, f: make(7), f()"),
        @"7"
    );
}

#[test]
fn fib() {
    assert_snapshot!(
        run("fib: (n) => if n < 2 n else fib(n-1) + fib(n-2), fib(10)"),
        @"55"
    );
}

#[test]
fn recursive_binding() {
    assert_snapshot!(
        run("a: 1 + b, b: 2, a"),
        @"3"
    );
}

#[test]
fn iterator_map() {
    assert_snapshot!(
        run("Iterator.range(0, 4).map((i) => i * 2).to_list"),
        @"[0, 2, 4, 6]"
    );
}

#[test]
fn iterator_filter() {
    assert_snapshot!(
        run("Iterator.range(0, 10).filter((i) => i % 2 = 0).to_list"),
        @"[0, 2, 4, 6, 8]"
    );
}

#[test]
fn errors_undefined_variable() {
    assert_snapshot!(run("foo + 1"), @"[resolve error] undefined variable: foo: not found in scope");
}

#[test]
fn errors_type_mismatch() {
    assert_snapshot!(
        run(r#"1 + "hello""#),
        @"[runtime error] expected number, got string: type mismatch"
    );
}

#[test]
fn errors_parse() {
    let out = run("x: 1 + + 2, x");
    assert!(out.starts_with("[parse error]"), "got: {}", out);
}

#[test]
fn errors_no_field() {
    assert_snapshot!(
        run("x: {a: 1}, x.b"),
        @"[runtime error] no such field: b: field not found"
    );
}
