#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/tool_one_arg.rs");
    t.pass("tests/ui/tool_with_cx.rs");
    t.pass("tests/ui/tool_unit_args.rs");
    t.compile_fail("tests/ui/structured_object.rs");
    t.compile_fail("tests/ui/tool_invalid.rs");
}
