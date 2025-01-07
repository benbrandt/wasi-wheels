//! Integration tests for the main CLI.

use assert_cmd::Command;
use wasi_wheels::{CPYTHON, WASI_SDK};

#[test]
fn install_build_tools() -> anyhow::Result<()> {
    let assert = Command::cargo_bin("wasi-wheels")?
        .arg("install-build-tools")
        .assert();

    assert.success();

    assert!(std::fs::read_dir(&*WASI_SDK)?.count() > 0);
    assert!(std::fs::read_dir(&*CPYTHON)?.count() > 0);

    Ok(())
}
