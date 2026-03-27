#[fittings::service]
trait WrongReturnTypeService {
    async fn hello(&self, params: ()) -> ();
}

fn main() {}
