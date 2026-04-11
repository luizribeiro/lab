#[fittings::service]
trait NonAsyncService {
    fn hello(&self, params: ()) -> Result<(), fittings::FittingsError>;
}

fn main() {}
