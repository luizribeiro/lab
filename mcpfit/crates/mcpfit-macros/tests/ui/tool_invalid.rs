use mcpfit::tool;

type Result<T> = mcpfit::Result<T>;

#[tool]
async fn missing_doc(args: ()) -> Result<()> {
    Ok(())
}

///
#[tool]
async fn empty_doc(args: ()) -> Result<()> {
    Ok(())
}

/// Not async.
#[tool]
fn not_async(args: ()) -> Result<()> {
    Ok(())
}

/// No return type.
#[tool]
async fn no_return(args: ()) {}

/// Zero args.
#[tool]
async fn zero_args() -> Result<()> {
    Ok(())
}

/// Too many args.
#[tool]
async fn three_args(a: (), b: (), c: ()) -> Result<()> {
    Ok(())
}

fn main() {}
