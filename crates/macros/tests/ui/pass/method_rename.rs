use fittings::FittingsError;

#[fittings::service]
trait MethodRenameService {
    #[fittings::method(name = "tools/list")]
    async fn list(&self, params: ()) -> Result<(), FittingsError>;
}

fn main() {}
