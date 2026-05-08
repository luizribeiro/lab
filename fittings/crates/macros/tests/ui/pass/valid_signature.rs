#[fittings::service]
trait ValidService {
    async fn hello(
        &self,
        ctx: fittings::ServiceContext,
        params: (),
    ) -> Result<(), fittings::FittingsError>;
}

fn main() {}
