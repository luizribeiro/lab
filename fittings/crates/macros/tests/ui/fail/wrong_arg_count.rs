#[fittings::service]
trait WrongArgCountService {
    async fn hello(&self) -> Result<(), fittings::FittingsError>;
}

fn main() {}
