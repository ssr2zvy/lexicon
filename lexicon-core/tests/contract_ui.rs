#[test]
fn compile_fail_contracts() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}

#[test]
fn compile_pass_contracts() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui-pass/*.rs");
}
