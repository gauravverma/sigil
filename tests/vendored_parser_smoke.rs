//! Phase 0 day 1: smoke test that the vendored parser compiles and produces
//! the same shape of output as codeix. Delete once Phase 0 day 6 switches
//! call sites over and codeix is removed.

#[test]
fn vendored_parser_extracts_rust_symbols() {
    let source = b"pub fn hello() -> u32 { 42 }\npub struct Foo;\n";
    let (symbols, _texts, _refs) =
        sigil::parser::treesitter::parse_file(source, "rust", "test.rs")
            .expect("vendored parser should parse trivial Rust");

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"hello"), "expected `hello` in {:?}", names);
    assert!(names.contains(&"Foo"), "expected `Foo` in {:?}", names);
}

#[test]
fn vendored_parser_extracts_python_symbols() {
    let source = b"def foo(x):\n    return x\n\nclass Bar:\n    pass\n";
    let (symbols, _texts, _refs) =
        sigil::parser::treesitter::parse_file(source, "python", "test.py")
            .expect("vendored parser should parse trivial Python");

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"foo"), "expected `foo` in {:?}", names);
    assert!(names.contains(&"Bar"), "expected `Bar` in {:?}", names);
}

#[test]
fn vendored_parser_extracts_typescript_symbols() {
    let source = b"export function greet(name: string): string { return `hi ${name}`; }\nexport class Agent {}\n";
    let (symbols, _texts, _refs) =
        sigil::parser::treesitter::parse_file(source, "typescript", "test.ts")
            .expect("vendored parser should parse trivial TypeScript");

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"greet"), "expected `greet` in {:?}", names);
    assert!(names.contains(&"Agent"), "expected `Agent` in {:?}", names);
}
