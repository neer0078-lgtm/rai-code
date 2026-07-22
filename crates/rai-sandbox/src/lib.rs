//! rai-sandbox — pluggable sandbox backends (OpenHands runtime pattern).
//!
//! - **Local** (default): `tokio::process` + AgentK approval (trusted dev).
//!   No isolation beyond the OS; for local/offline mode.
//! - **E2B** (feature `e2b`): Firecracker microVM, ~150ms cold start, runs the
//!   app dev server + headless Chromium TOGETHER, CDP on :9222, public URL,
//!   pause/resume preserves memory. Via REST API (stub in T33; live wiring later).
//! - **Daytona** (feature `daytona`): Docker/OCI, persistent stateful workspaces,
//!   GPU, audit logs (future).
#![warn(missing_docs)]

pub mod sandbox;

#[cfg(feature = "e2b")]
pub use sandbox::e2b::{E2bConfig, E2bSandbox};
pub use sandbox::{LocalSandbox, Sandbox, SandboxOutput};
