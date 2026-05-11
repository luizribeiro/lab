//! Tool-schema catalog (scope §OP2 items 1, 7).
//!
//! Built once at `rfl chat` startup from each plugin's
//! `openrpc.json`. Served by `CorePluginService` over
//! `core.tools_list` to the provider connection.

use std::collections::BTreeMap;
use std::path::PathBuf;
#[cfg(any(test, feature = "test-fixture"))]
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::broker_acl::BrokerAcl;
use crate::compile::CompiledPlugin;
use crate::lock::CanonicalId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSchema {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters_schema: Value,
}

#[derive(Debug)]
pub struct ToolSchemaCatalog {
    schemas: Vec<ToolSchema>,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolCatalogError {
    #[error(
        "plugin {canonical} declares tool {tool:?} but its openrpc.json has no matching method"
    )]
    ToolMissingOpenRpcMethod {
        canonical: CanonicalId,
        tool: String,
    },
    #[error("plugin {canonical} openrpc.json failed to parse: {source}")]
    OpenRpcParseError {
        canonical: CanonicalId,
        #[source]
        source: serde_json::Error,
    },
    #[error("plugin {canonical} openrpc.json missing or unreadable at {path:?}: {source}")]
    OpenRpcReadError {
        canonical: CanonicalId,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("plugin {canonical} has no package directory entry")]
    MissingPackageDir { canonical: CanonicalId },
}

#[derive(Debug, Deserialize)]
struct OpenRpcDoc {
    #[serde(default)]
    methods: Vec<OpenRpcMethod>,
}

#[derive(Debug, Deserialize)]
struct OpenRpcMethod {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    params: Vec<OpenRpcParam>,
}

#[derive(Debug, Deserialize)]
struct OpenRpcParam {
    name: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    schema: Value,
}

impl ToolSchemaCatalog {
    pub fn build(
        acl: &BrokerAcl,
        compiled: &BTreeMap<CanonicalId, CompiledPlugin>,
        package_dirs: &BTreeMap<CanonicalId, PathBuf>,
    ) -> Result<Self, ToolCatalogError> {
        let mut schemas: Vec<ToolSchema> = Vec::new();

        for canonical in compiled.keys() {
            let tools_for_plugin: Vec<&String> = acl
                .tool_routes
                .iter()
                .filter_map(|(t, c)| if c == canonical { Some(t) } else { None })
                .collect();
            if tools_for_plugin.is_empty() {
                continue;
            }

            let pkg_dir =
                package_dirs
                    .get(canonical)
                    .ok_or_else(|| ToolCatalogError::MissingPackageDir {
                        canonical: canonical.clone(),
                    })?;
            let openrpc_path = pkg_dir.join("openrpc.json");
            let raw = std::fs::read_to_string(&openrpc_path).map_err(|source| {
                ToolCatalogError::OpenRpcReadError {
                    canonical: canonical.clone(),
                    path: openrpc_path.clone(),
                    source,
                }
            })?;
            let doc: OpenRpcDoc = serde_json::from_str(&raw).map_err(|source| {
                ToolCatalogError::OpenRpcParseError {
                    canonical: canonical.clone(),
                    source,
                }
            })?;

            let mut by_name: BTreeMap<&str, &OpenRpcMethod> = BTreeMap::new();
            for m in &doc.methods {
                by_name.insert(m.name.as_str(), m);
            }

            for tool in tools_for_plugin {
                let method = by_name.get(tool.as_str()).ok_or_else(|| {
                    ToolCatalogError::ToolMissingOpenRpcMethod {
                        canonical: canonical.clone(),
                        tool: tool.clone(),
                    }
                })?;
                schemas.push(synthesise_schema(tool, method));
            }
        }

        schemas.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { schemas })
    }

    pub fn list(&self) -> &[ToolSchema] {
        &self.schemas
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn empty_for_tests() -> Arc<Self> {
        Arc::new(Self {
            schemas: Vec::new(),
        })
    }
}

fn synthesise_schema(name: &str, method: &OpenRpcMethod) -> ToolSchema {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();
    for param in &method.params {
        properties.insert(param.name.clone(), param.schema.clone());
        if param.required {
            required.push(Value::String(param.name.clone()));
        }
    }
    let parameters_schema = json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": Value::Array(required),
    });
    ToolSchema {
        name: name.to_string(),
        description: method.description.clone(),
        parameters_schema,
    }
}
