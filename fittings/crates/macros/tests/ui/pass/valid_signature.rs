#[fittings::service]
trait ValidService {
    async fn hello(&self, params: ()) -> Result<(), fittings::FittingsError>;
}

fn main() {}
