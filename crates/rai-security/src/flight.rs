//! The hash-chained flight recorder — a tamper-evident log of every agent
//! action. Each entry's hash = `sha256(prev_hash || entry_json)`; flipping any
//! byte breaks the chain.
//!
//! Clean-room port of the AgentK pattern (github.com/Atomics-hub/agentk, MIT):
//! a hash-chained JSONL log. RAI Code's own naming + structure (FlightRecorder,
//! FlightEntry, tip_hex, verify_chain) — no literal code reproduced.

use crate::kernel::{Syscall, TaintLabel};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The zero hash (the chain's genesis tip — the hash before any entry).
pub const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A single flight-log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightEntry {
    /// The sequence number (0-indexed).
    pub seq: u64,
    /// The kind of action (e.g. "tool_call", "syscall").
    pub kind: String,
    /// The action payload (tool name + args, or syscall).
    pub action: serde_json::Value,
    /// The taint label of the action's data.
    pub taint: TaintLabel,
    /// The hash of the previous entry (the chain link).
    pub prev_hash: String,
    /// The hash of this entry (`sha256(prev_hash || entry_json_without_hash)`).
    pub hash: String,
    /// A monotonic timestamp (nanos since UNIX_EPOCH) — for ordering, not crypto.
    pub timestamp_nanos: u128,
}

/// The hash-chained flight recorder.
///
/// Appends entries to an in-memory `Vec`; each entry's hash chains to the
/// previous. `verify_chain()` returns `true` for an unmodified chain, `false`
/// if any byte is flipped. (A persistent JSONL writer lands in a later task;
/// T26 is the in-memory chain + the tamper-evidence check.)
/// The hash-chained flight recorder.
///
/// Appends entries to an in-memory chain; each entry's hash chains to the
/// previous. `verify_chain()` returns `true` for an unmodified chain, `false`
/// if any byte is flipped. The entries live behind a `Mutex` so the
/// `SecurityKernel` trait's `&self` methods can append (interior mutability).
/// (A persistent JSONL writer lands in a later task; T26 is the in-memory
/// chain + the tamper-evidence check.)
#[derive(Debug, Default)]
pub struct FlightRecorder {
    inner: Mutex<FlightInner>,
}

#[derive(Debug, Default)]
struct FlightInner {
    /// The chain of entries.
    entries: Vec<FlightEntry>,
    /// The current tip hash (the hash of the last entry, or ZERO_HASH if empty).
    tip: String,
}

impl FlightRecorder {
    /// Construct an empty recorder (tip = ZERO_HASH).
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FlightInner {
                entries: vec![],
                tip: ZERO_HASH.into(),
            }),
        }
    }

    /// The current tip as a hex string.
    pub fn tip_hex(&self) -> String {
        self.inner.lock().tip.clone()
    }

    /// A snapshot of the entries (cloned) — for inspection/tests.
    pub fn entries(&self) -> Vec<FlightEntry> {
        self.inner.lock().entries.clone()
    }

    /// A mutable snapshot of the entries (cloned) — for tamper tests.
    pub fn entries_mut_snapshot(&self) -> Vec<FlightEntry> {
        self.entries()
    }

    /// Append a tool-call entry. Returns the new entry's hash.
    pub fn append_tool_call(
        &self,
        tool: &str,
        args: &serde_json::Value,
        taint: TaintLabel,
    ) -> anyhow::Result<String> {
        let action = serde_json::json!({
            "tool": tool,
            "args": args,
        });
        self.append("tool_call", action, taint)
    }

    /// Append a syscall entry.
    pub fn append_syscall(&self, syscall: &Syscall, taint: TaintLabel) -> anyhow::Result<String> {
        let action = serde_json::to_value(syscall)?;
        self.append("syscall", action, taint)
    }

    /// Append a generic entry (the core chain-extension). Takes &self (the
    /// Mutex provides interior mutability so SecurityKernel's &self methods can
    /// append).
    pub fn append(
        &self,
        kind: &str,
        action: serde_json::Value,
        taint: TaintLabel,
    ) -> anyhow::Result<String> {
        let mut inner = self.inner.lock();
        let seq = inner.entries.len() as u64;
        let prev_hash = inner.tip.clone();
        let timestamp_nanos = now_nanos();
        let pre_image = serde_json::json!({
            "seq": seq,
            "kind": kind,
            "action": action,
            "taint": taint,
            "prev_hash": prev_hash,
            "timestamp_nanos": timestamp_nanos,
        });
        let pre_json = serde_json::to_string(&pre_image)?;
        let hash = hash_pair(&prev_hash, &pre_json);

        let entry = FlightEntry {
            seq,
            kind: kind.into(),
            action,
            taint,
            prev_hash,
            hash: hash.clone(),
            timestamp_nanos,
        };
        inner.entries.push(entry);
        inner.tip = hash.clone();
        Ok(hash)
    }

    /// Verify the chain: recompute every entry's hash + check the links.
    /// Returns true iff the chain is unmodified.
    pub fn verify_chain(&self) -> bool {
        let inner = self.inner.lock();
        let mut expected_prev = ZERO_HASH.to_string();
        for entry in &inner.entries {
            if entry.prev_hash != expected_prev {
                return false;
            }
            let pre_image = serde_json::json!({
                "seq": entry.seq,
                "kind": entry.kind,
                "action": entry.action,
                "taint": entry.taint,
                "prev_hash": entry.prev_hash,
                "timestamp_nanos": entry.timestamp_nanos,
            });
            let pre_json = match serde_json::to_string(&pre_image) {
                Ok(s) => s,
                Err(_) => return false,
            };
            let recomputed = hash_pair(&entry.prev_hash, &pre_json);
            if recomputed != entry.hash {
                return false;
            }
            expected_prev = entry.hash.clone();
        }
        // The tip should match the last entry's hash (or ZERO_HASH if empty).
        expected_prev == inner.tip
    }
}

/// `sha256(prev_hash_bytes || preimage_bytes)` as a hex string.
fn hash_pair(prev_hash: &str, preimage: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(preimage.as_bytes());
    hex_encode(&hasher.finalize())
}

/// A minimal hex encoder (avoids pulling a `hex` dep just for this).
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Monotonic nanos since UNIX_EPOCH (for ordering, not crypto).
fn now_nanos() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Syscall;

    /// T26: append 3 entries, verify_chain true; flip a byte, verify false.
    #[test]
    fn flight_recorder_chain_integrity() {
        let rec = FlightRecorder::new();
        assert_eq!(rec.tip_hex(), ZERO_HASH);

        rec.append_tool_call(
            "Read",
            &serde_json::json!({"path":"a.rs"}),
            TaintLabel::Clean,
        )
        .unwrap();
        rec.append_tool_call(
            "Edit",
            &serde_json::json!({"path":"a.rs"}),
            TaintLabel::Clean,
        )
        .unwrap();
        rec.append_syscall(
            &Syscall::RunShell {
                command: "echo hi".into(),
            },
            TaintLabel::Clean,
        )
        .unwrap();

        assert_eq!(rec.entries().len(), 3);
        // Each entry's prev_hash links to the prior entry's hash (or ZERO_HASH
        // for the first).
        let entries = rec.entries();
        assert_eq!(entries[0].prev_hash, ZERO_HASH);
        assert_eq!(entries[1].prev_hash, entries[0].hash);
        assert_eq!(entries[2].prev_hash, entries[1].hash);
        // The tip is the last entry's hash.
        assert_eq!(rec.tip_hex(), entries[2].hash);
        // The unmodified chain verifies.
        assert!(rec.verify_chain());

        // Flip a byte in the middle entry's action -> chain breaks.
        // The entries are behind a Mutex; to simulate tampering, we inject a
        // tampered entry directly via the inner lock (a test-only path that
        // emulates an attacker editing the on-disk log).
        {
            let mut inner = rec.inner.lock();
            // The action for a tool_call is {"tool":.., "args":{..}}; the path
            // is at action.args.path. Tamper it.
            if let Some(args) = inner.entries[1].action.get_mut("args") {
                if let Some(action) = args.get_mut("path") {
                    if let Some(s) = action.as_str() {
                        let mut chars: Vec<char> = s.chars().collect();
                        if !chars.is_empty() {
                            chars[0] = if chars[0] == 'a' { 'b' } else { 'a' };
                        }
                        let new_s: String = chars.into_iter().collect();
                        *action = serde_json::Value::String(new_s);
                    }
                }
            }
        }
        assert!(
            !rec.verify_chain(),
            "tampering with an entry's action must break the chain"
        );
    }

    /// T26: the tip is the zero hash when empty; non-zero after an append.
    #[test]
    fn flight_recorder_tip_progression() {
        let rec = FlightRecorder::new();
        assert_eq!(rec.tip_hex(), ZERO_HASH);
        rec.append("note", serde_json::json!({"x":1}), TaintLabel::Clean)
            .unwrap();
        assert_ne!(rec.tip_hex(), ZERO_HASH);
        assert!(rec.verify_chain());
    }

    /// T26: seq numbers are 0-indexed and monotonic.
    #[test]
    fn flight_recorder_seq_monotonic() {
        let rec = FlightRecorder::new();
        for i in 0..5 {
            rec.append("note", serde_json::json!({"i": i}), TaintLabel::Clean)
                .unwrap();
        }
        for (i, e) in rec.entries().iter().enumerate() {
            assert_eq!(e.seq, i as u64);
        }
        assert!(rec.verify_chain());
    }

    /// T26: a tainted entry is recorded with its taint label.
    #[test]
    fn flight_recorder_records_taint() {
        let rec = FlightRecorder::new();
        rec.append_tool_call(
            "Net",
            &serde_json::json!({"token":"x"}),
            TaintLabel::UserSecret,
        )
        .unwrap();
        assert_eq!(rec.entries()[0].taint, TaintLabel::UserSecret);
        assert!(rec.verify_chain());
    }
}
