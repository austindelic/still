//! Source selection, item-kind inference, and source registry composition.

pub mod infer;
pub mod registries;
pub mod registry;
pub mod source;

pub use infer::{default_backend, infer_item_kind_from_backend, normalize_auto_backend};
pub use registry::{CompiledSource, SourceRegistry};
pub use source::{SourceCapability, SourceId, SourceIntent, SourceResolver, SourceSelection};
