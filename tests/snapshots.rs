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
    assert_snapshot!(run("1 == 1"), @"true");
    assert_snapshot!(run("1 != 2"), @"true");
    assert_snapshot!(run("3 > 2"), @"true");
    assert_snapshot!(run("3 <= 3"), @"true");
}

#[test]
fn bool_literals() {
    assert_snapshot!(run("true"), @"true");
    assert_snapshot!(run("false"), @"false");
    assert_snapshot!(run("!true"), @"false");
    assert_snapshot!(run("true && false"), @"false");
    assert_snapshot!(run("true || false"), @"true");
}

#[test]
fn short_circuit() {
    // && returns falsy lhs without evaluating rhs (rhs would be runtime error)
    assert_snapshot!(run(r#"false && (1 + "wrong")"#), @"false");
    // || returns truthy lhs without evaluating rhs
    assert_snapshot!(run(r#"true || (1 + "wrong")"#), @"true");
}

#[test]
fn if_then_else() {
    assert_snapshot!(run("if 1 < 2 then \"yes\" else \"no\""), @r###""yes""###);
    assert_snapshot!(run("if 1 > 2 then \"yes\" else \"no\""), @r###""no""###);
}

#[test]
fn binds_and_blocks() {
    assert_snapshot!(run("x: 5, x + 10"), @"15");
    assert_snapshot!(run("x: {a: 1, b: 2}, x.a + x.b"), @"3");
    assert_snapshot!(run("x: {a: 1}, x[\"a\"]"), @"1");
}

#[test]
fn quoted_keys() {
    assert_snapshot!(run(r#"{"a": 1, "b": 2}.a"#), @"1");
    assert_snapshot!(run(r#"x: {"first": 10, "second": 20}, x["first"] + x["second"]"#), @"30");
}

#[test]
fn json_literal() {
    // a pure JSON object should evaluate to itself (formatted)
    assert_snapshot!(
        run(r#"{"name": "Alice", "age": 30, "active": true}"#),
        @r###"{"active": true, "age": 30, "name": "Alice"}"###
    );
}

#[test]
fn import_basic() {
    use std::fs;
    let dir = std::env::temp_dir().join(format!("spctr-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("lib.spc"), "{\n  add: (a, b) => a + b\n}\n").unwrap();
    let path = dir.join("lib.spc");
    let src = format!(r#"m: import("{}"), m.add(3, 4)"#, path.display());
    assert_snapshot!(run(&src), @"7");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn string_escapes() {
    assert_snapshot!(run(r#""hello\nworld""#), @r###""hello\nworld""###);
    assert_snapshot!(run(r#""quote: \"hi\"""#), @r###""quote: \"hi\"""###);
    assert_snapshot!(run(r#""é""#), @r###""é""###);
}

#[test]
fn string_interpolation() {
    assert_snapshot!(
        run(r#"name: "world", "hello ${name}!""#),
        @r###""hello world!""###
    );
    assert_snapshot!(
        run(r#"a: "X", b: "Y", "${a}-${b}-${a}""#),
        @r###""X-Y-X""###
    );
    // `\$` escapes a literal dollar sign so `${...}` can be written literally.
    assert_snapshot!(
        run(r#""price: \${5}""#),
        @r###""price: ${5}""###
    );
    // Interp expression itself contains a nested string with its own interp.
    assert_snapshot!(
        run(r#"a: "X", "outer ${if a == "X" then "yes ${a}" else "no"} done""#),
        @r###""outer yes X done""###
    );
    // Empty literal fragments on either side of an interp.
    assert_snapshot!(
        run(r#"a: "X", "${a}""#),
        @r###""X""###
    );
}

#[test]
fn comments() {
    assert_snapshot!(run("// comment\n1 + 2"), @"3");
    assert_snapshot!(run("/* block */ 1 + 2"), @"3");
    assert_snapshot!(run("x: 1, /* mid */ y: 2, /*tail*/ x + y"), @"3");
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
        run("fib: (n) => if n < 2 then n else fib(n-1) + fib(n-2), fib(10)"),
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
fn list_range_map() {
    assert_snapshot!(
        run("List.map(List.range(0, 4), (i) => i * 2)"),
        @"[0, 2, 4, 6]"
    );
}

#[test]
fn list_filter() {
    assert_snapshot!(
        run("List.filter(List.range(0, 10), (i) => i % 2 == 0)"),
        @"[0, 2, 4, 6, 8]"
    );
}

#[test]
fn list_reduce() {
    assert_snapshot!(
        run("List.reduce(List.range(1, 6), 0, (acc, x) => acc + x)"),
        @"15"
    );
}

#[test]
fn number_ops() {
    assert_snapshot!(run("Number.sqrt(Number.pow(3, 2) + Number.pow(4, 2))"), @"5");
    assert_snapshot!(run(r#"Number.parse("42") + 1"#), @"43");
    assert_snapshot!(run("Number.toString(3.14)"), @r###""3.14""###);
}

#[test]
fn string_ops() {
    assert_snapshot!(run(r#"String.length("hello")"#), @"5");
    assert_snapshot!(
        run(r#"String.split("a,b,c", ",")"#),
        @r###"["a", "b", "c"]"###
    );
    assert_snapshot!(run(r#"String.contains("hello world", "world")"#), @"true");
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
