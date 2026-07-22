//! rai-python — the bridge to Graphiti (temporal code KG) + Hindsight (user model).
//!
//! Both are Python. The MemoryStore trait has two implementations (same interface):
//!  - SidecarMemoryStore (Phase 1, default): spawn a Python sidecar process, JSON-RPC
//!    over stdin/stdout. Simple, crash-isolated, no GIL/deadlock risk, IPC overhead
//!    ~0.1ms (negligible vs Graphiti's seconds-per-episode). RECOMMENDED START.
//!  - PyO3MemoryStore (Phase 2, opt-in feature `pyo3-embed`): embed Python in-process
//!    via PyO3, dedicated Python thread + channel bridge. Single-process deployment.
#![warn(missing_docs)]

pub mod graphiti;
pub mod hindsight;
pub mod store;

pub use graphiti::Episode;
pub use hindsight::{Disposition, MemoryBankConfig};
pub use store::{
    parse_request, JsonRpcError, JsonRpcRequest, JsonRpcResponse, MemoryStore, MockMemoryStore,
    SidecarSpawnConfig,
};
