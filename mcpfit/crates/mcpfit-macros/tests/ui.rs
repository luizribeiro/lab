#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/tool_one_arg.rs");
    t.compile_fail("tests/ui/structured_object.rs");
}
