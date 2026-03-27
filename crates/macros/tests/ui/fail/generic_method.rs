#[fittings::service]
trait GenericMethodService {
    async fn hello<T>(&self, params: T) -> Result<(), fittings::FittingsError>;
}

fn main() {}
