#[fittings::service]
trait MissingFittingsErrorService {
    async fn hello(&self, params: ()) -> Result<(), String>;
}

fn main() {}
