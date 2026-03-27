#[fittings::service]
trait MutSelfReceiverService {
    async fn hello(&mut self, params: ()) -> Result<(), fittings::FittingsError>;
}

fn main() {}
