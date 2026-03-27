use async_trait::async_trait;

use crate::{
    error::FittingsError,
    message::{Request, Response},
};

#[async_trait]
pub trait Service: Send + Sync {
    async fn call(&self, req: Request) -> Result<Response, FittingsError>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::json;

    use crate::{
        error::FittingsError,
        message::{Request, Response},
    };

    use super::Service;

    struct EchoService;

    #[async_trait]
    impl Service for EchoService {
        async fn call(&self, req: Request) -> Result<Response, FittingsError> {
            Ok(Response {
                id: req.id,
                result: req.params,
                metadata: Default::default(),
            })
        }
    }

    fn assert_service_impl<T: Service>() {}

    #[test]
    fn service_trait_is_implementable() {
        assert_service_impl::<EchoService>();

        let _request = Request {
            id: "1".into(),
            method: "echo".into(),
            params: json!({"x": 1}),
            metadata: Default::default(),
        };
    }
}
