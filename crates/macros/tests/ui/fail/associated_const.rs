#[fittings::service]
trait HasAssociatedConst {
    const VERSION: &'static str;

    async fn hello(&self, params: ()) -> Result<(), fittings::FittingsError>;
}

fn main() {}
