#[fittings::service]
trait MissingMethodNameService {
    #[fittings::method]
    async fn hello(&self, params: ()) -> Result<(), fittings::FittingsError>;
}

fn main() {}
