//! T65: docs branding test — verifies README, AGENTS, LICENSE, BUILD, ITVF-LOOP,
//! and TASKS contain the RAI Labs branding + license + contact info.

use std::fs;
use std::path::PathBuf;

/// The workspace root (3 levels up from crates/rai-cli/).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("should canonicalize to workspace root")
}

#[test]
fn docs_branding_present() {
    let root = workspace_root();
    let files: Vec<(&str, &[&str])> = vec![
        (
            "README.md",
            &["RAI Labs", "railabs.in", "reach@railabs.in", "Apache-2.0"],
        ),
        ("AGENTS.md", &["RAI Labs", "railabs.in", "reach@railabs.in"]),
        ("LICENSE", &["RAI Labs", "Apache"]),
        ("BUILD.md", &["RAI Labs", "railabs.in", "reach@railabs.in"]),
        ("docs/ITVF-LOOP.md", &["RAI Labs", "railabs.in"]),
        ("docs/UX-DESIGN.md", &["RAI Labs", "railabs.in"]),
    ];

    for (path, required_strings) in &files {
        let content = fs::read_to_string(root.join(path))
            .unwrap_or_else(|_| panic!("should be able to read {path}"));
        for required in *required_strings {
            assert!(
                content.contains(required),
                "{path} should contain '{required}':\n--- first 200 chars ---\n{}",
                &content[..content.len().min(200)]
            );
        }
    }

    // The references/claude-code/ read-only marker exists.
    let readonly_marker = fs::metadata(root.join("references/claude-code/.RAI-READONLY"));
    assert!(
        readonly_marker.is_ok(),
        "references/claude-code/.RAI-READONLY should exist"
    );

    // The Python sidecar exists + has the branding.
    let sidecar =
        fs::read_to_string(root.join("scripts/sidecar.py")).expect("should read sidecar.py");
    assert!(
        sidecar.contains("RAI Labs"),
        "sidecar.py should contain 'RAI Labs'"
    );

    println!("Docs branding: all checks passed ✓");
}
