use mcpfit_macros::StructuredObject;

#[derive(StructuredObject)]
enum NotAStruct {
    A,
    B,
}

#[derive(StructuredObject)]
union AlsoNot {
    a: u32,
    b: u32,
}

fn main() {}
