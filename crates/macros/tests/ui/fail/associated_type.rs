#[fittings::service]
trait HasAssociatedType {
    type Response;

    async fn hello(&self, params: ()) -> Result<(), fittings::FittingsError>;
}

fn main() {}
