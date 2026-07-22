//! rai-security — the execution-layer security kernel.
//!
//! - AgentK pattern: typed syscalls, taint labels, flight recorder.
//! - PACT-inspired argument-level provenance: value origin tracking + trust contracts.
#![warn(missing_docs)]

pub mod flight;
pub mod kernel;
pub mod provenance;

pub use flight::{FlightEntry, FlightRecorder, ZERO_HASH};
pub use kernel::{taint_of, DefaultKernel, SecurityDecision, SecurityKernel, Syscall, TaintLabel};
pub use provenance::{ProvenanceChecker, TrackedValue, TrustContract, ValueOrigin};
