use async_trait::async_trait;
use fittings_core::{
    error::FittingsError,
    message::{Metadata, Request, Response},
    service::Service,
};
use serde_json::Value;

#[async_trait]
pub trait MethodRouter: Send + Sync {
    async fn route(
        &self,
        method: &str,
        params: Value,
        metadata: Metadata,
    ) -> Result<Value, FittingsError>;
}

pub struct RouterService<R> {
    router: R,
}

impl<R> RouterService<R> {
    pub fn new(router: R) -> Self {
        Self { router }
    }
}

#[async_trait]
impl<R> Service for RouterService<R>
where
    R: MethodRouter,
{
    async fn call(&self, req: Request) -> Result<Response, FittingsError> {
        let result = self
            .router
            .route(&req.method, req.params, req.metadata.clone())
            .await?;

        Ok(Response {
            id: req.id,
            result,
            metadata: req.metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::json;

    use fittings_core::{error::FittingsError, message::Request, service::Service};

    use super::{MethodRouter, RouterService};

    struct EchoRouter;

    #[async_trait]
    impl MethodRouter for EchoRouter {
        async fn route(
            &self,
            method: &str,
            params: serde_json::Value,
            _metadata: fittings_core::message::Metadata,
        ) -> Result<serde_json::Value, FittingsError> {
            if method != "echo" {
                return Err(FittingsError::method_not_found(method.to_string()));
            }

            Ok(params)
        }
    }

    #[tokio::test]
    async fn router_service_delegates_and_wraps_response() {
        let service = RouterService::new(EchoRouter);
        let request = Request {
            id: "r-1".to_string(),
            method: "echo".to_string(),
            params: json!({"x": 1}),
            metadata: Default::default(),
        };

        let response = service.call(request).await.expect("call should succeed");

        assert_eq!(response.id, "r-1");
        assert_eq!(response.result, json!({"x": 1}));
    }

    #[tokio::test]
    async fn router_service_propagates_router_errors() {
        let service = RouterService::new(EchoRouter);
        let request = Request {
            id: "r-2".to_string(),
            method: "unknown".to_string(),
            params: json!({}),
            metadata: Default::default(),
        };

        let error = service.call(request).await.expect_err("call should fail");
        assert!(matches!(
            error,
            FittingsError::MethodNotFound(message) if message == "unknown"
        ));
    }
}
