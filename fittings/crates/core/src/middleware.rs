use async_trait::async_trait;

use crate::{
    error::FittingsError,
    message::{Request, Response},
    service::Service,
};

#[async_trait]
pub trait Middleware: Send + Sync {
    async fn handle(&self, req: Request, next: &dyn Service) -> Result<Response, FittingsError>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::{
        error::FittingsError,
        message::{Request, Response},
        service::Service,
    };

    use super::Middleware;

    struct Passthrough;

    #[async_trait]
    impl Middleware for Passthrough {
        async fn handle(
            &self,
            req: Request,
            next: &dyn Service,
        ) -> Result<Response, FittingsError> {
            next.call(req).await
        }
    }

    fn assert_middleware_impl<T: Middleware>() {}

    #[test]
    fn middleware_trait_is_implementable() {
        assert_middleware_impl::<Passthrough>();
    }
}
