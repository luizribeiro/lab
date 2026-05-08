use fittings::{FittingsError, ServiceContext};

#[fittings::service]
trait MethodRenameService {
    #[fittings::method(name = "tools/list")]
    async fn list(&self, ctx: ServiceContext, params: ()) -> Result<(), FittingsError>;
}

fn main() {}
