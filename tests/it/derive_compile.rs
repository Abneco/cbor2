//! Real consumer crates catch expansion failures hidden by this crate's
//! dev-dependencies and test-only imports.

#[test]
fn derive_consumer_fixtures() {
    use std::{path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = root.join("tests/derive-fixtures/Cargo.toml");
    let target = root.join("target/derive-fixtures");
    for (binary, expected_error) in [
        ("pass", None),
        ("array_skip", Some("array field positions")),
        ("tuple_skip", Some("array field positions")),
    ] {
        let output = Command::new(env!("CARGO"))
            .args([
                if expected_error.is_none() {
                    "run"
                } else {
                    "check"
                },
                "--offline",
                "--manifest-path",
            ])
            .arg(&manifest)
            .arg("--target-dir")
            .arg(&target)
            .args(["--bin", binary])
            .output()
            .expect("cargo is available to run consumer fixtures");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        if let Some(expected) = expected_error {
            assert!(
                !output.status.success() && diagnostic.contains(expected),
                "{binary}: {diagnostic}"
            );
        } else {
            assert!(output.status.success(), "{binary}: {diagnostic}");
        }
    }
}
