//! Per-connection `core.tools_list` service (scope §OP2 items 2, 4).
//!
//! Composed by the supervisor only for provider connections; the
//! supervisor's connection-service routing returns
//! `MethodNotFound` for non-providers (scope §OP2 item 5).

use std::sync::Arc;

use async_trait::async_trait;
use fittings_core::context::ServiceContext;
use fittings_core::error::FittingsError;
use fittings_core::message::{JsonRpcId, Request, Response};
use fittings_core::service::Service;
use serde_json::json;

use crate::supervisor::tool_catalog::ToolSchemaCatalog;

pub struct CorePluginService {
    pub catalog: Arc<ToolSchemaCatalog>,
}

#[async_trait]
impl Service for CorePluginService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        if req.method == "core.tools_list" {
            let tools = self.catalog.list().to_vec();
            let id = req.id.unwrap_or(JsonRpcId::Null);
            return Ok(Response {
                id,
                result: json!({ "tools": tools }),
                metadata: Default::default(),
            });
        }
        Err(FittingsError::method_not_found(req.method))
    }
}
