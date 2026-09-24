//! Standard org.varlink.service introspection interface implementation.
//!
//! Provides service identity and human-readable interface schema definitions.

use super::protocol::VarlinkReply;
use serde_json::json;

/// Varlink interface definition text for org.varlink.service.
pub const ORG_VARLINK_SERVICE_INTERFACE: &str = r#"
interface org.varlink.service

method GetInfo() -> (
  vendor: string,
  product: string,
  version: string,
  url: string,
  interfaces: []string
)

method GetInterfaceDescription(interface: string) -> (description: string)

error InterfaceNotFound(interface: string)
error MethodNotFound(method: string)
error MethodNotImplemented(method: string)
error InvalidParameter(parameter: string)
"#;

/// Varlink interface definition text for io.syntrop.Model1.
pub const IO_SYNTROP_MODEL1_INTERFACE: &str = r#"
interface io.syntrop.Model1

type ModelEntry (
  id: string,
  digest: string,
  name: ?string,
  tag: ?string,
  size_bytes: int,
  pinned: bool,
  format: string
)

method List() -> (models: []ModelEntry)
method Inspect(id: string) -> (info: ModelEntry, metadata: ?string)
method Pin(id: string) -> ()
method Unpin(id: string) -> ()
method Prune(max_bytes: int) -> (reclaimed_bytes: int)
method GetStorageStats() -> (total_bytes: int, model_count: int, pinned_count: int)

error NoSuchModel(id: string)
error InvalidIdentifier(id: string)
error OperationFailed(reason: string)
"#;

/// Handles standard org.varlink.service method dispatches.
pub fn handle_service_call(method: &str, params: Option<&serde_json::Value>) -> Option<VarlinkReply> {
    match method {
        "org.varlink.service.GetInfo" => Some(VarlinkReply::ok(json!({
            "vendor": "Syntropd Project",
            "product": "modeld",
            "version": "0.1.0",
            "url": "https://github.com/syntropd/modeld",
            "interfaces": [
                "org.varlink.service",
                "io.syntrop.Model1"
            ]
        }))),
        "org.varlink.service.GetInterfaceDescription" => {
            let iface = params
                .and_then(|p| p.get("interface"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match iface {
                "org.varlink.service" => Some(VarlinkReply::ok(json!({
                    "description": ORG_VARLINK_SERVICE_INTERFACE.trim()
                }))),
                "io.syntrop.Model1" => Some(VarlinkReply::ok(json!({
                    "description": IO_SYNTROP_MODEL1_INTERFACE.trim()
                }))),
                _ => Some(VarlinkReply::err(
                    "org.varlink.service.InterfaceNotFound",
                    Some(json!({ "interface": iface })),
                )),
            }
        }
        _ => None,
    }
}
