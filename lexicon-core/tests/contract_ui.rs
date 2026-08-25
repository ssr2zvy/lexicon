#[test]
fn compile_fail_contracts() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
