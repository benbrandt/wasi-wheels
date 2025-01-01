//! Build script for pydantic wheel

use std::{env, path::PathBuf, process::Command};

use wasi_wheels::run;

fn main() -> anyhow::Result<()> {
    let repo_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cpython_dir = repo_dir.join("cpython-3.12.8");
    let pydantic_dir = repo_dir.join("pydantic_core-2.27.1");

    run(Command::new("maturin")
        .current_dir(pydantic_dir)
        .envs([
            (
                "PYTHONPATH",
                cpython_dir
                    .join("builddir/wasi/install/lib/python3.12")
                    .to_str()
                    .unwrap(),
            ),
            (
                "_PYTHON_SYSCONFIGDATA_NAME",
                "_sysconfigdata__wasi_wasm32-wasi",
            ),
        ])
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasi",
            "-i",
            "python3.12",
            "-vv",
        ]))?;

    Ok(())
}
