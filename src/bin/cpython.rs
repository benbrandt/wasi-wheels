//! Prep cpython wasi build

use std::{env, fs, iter, path::PathBuf, process::Command};

use anyhow::{bail, Context};

#[cfg(any(target_os = "macos", target_os = "windows"))]
const PYTHON_EXECUTABLE: &str = "python.exe";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PYTHON_EXECUTABLE: &str = "python";

fn main() -> anyhow::Result<()> {
    let repo_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cpython_dir = repo_dir.join("cpython-3.13.1");
    let cpython_wasi_dir = cpython_dir.join("builddir/wasi");
    let cpython_native_dir = cpython_dir.join("builddir/build");
    if !cpython_wasi_dir.join("libpython3.13.so").exists()
        && !cpython_wasi_dir.join("libpython3.13.a").exists()
    {
        if !cpython_native_dir.join(PYTHON_EXECUTABLE).exists() {
            fs::create_dir_all(&cpython_native_dir)?;
            fs::create_dir_all(&cpython_wasi_dir)?;

            run(Command::new("../../configure")
                .current_dir(&cpython_native_dir)
                .arg(format!(
                    "--prefix={}/install",
                    cpython_native_dir.to_str().unwrap()
                )))?;

            run(Command::new("make").current_dir(&cpython_native_dir))?;
        }

        let config_guess = run(Command::new("../../config.guess").current_dir(&cpython_wasi_dir))?;

        run(Command::new("../../Tools/wasm/wasi-env")
            .env("CONFIG_SITE", "../../Tools/wasm/config.site-wasm32-wasi")
            .env("CFLAGS", "-fPIC")
            .current_dir(&cpython_wasi_dir)
            .args([
                "../../configure",
                "-C",
                "--host=wasm32-unknown-wasi",
                &format!("--build={}", String::from_utf8(config_guess)?),
                &format!(
                    "--with-build-python={}/{PYTHON_EXECUTABLE}",
                    cpython_native_dir.to_str().unwrap()
                ),
                &format!("--prefix={}/install", cpython_wasi_dir.to_str().unwrap()),
                "--disable-test-modules",
            ]))?;

        run(Command::new("make")
            .current_dir(&cpython_wasi_dir)
            .arg("install"))?;
    }

    Ok(())
}

fn run(command: &mut Command) -> anyhow::Result<Vec<u8>> {
    let command_string = iter::once(command.get_program())
        .chain(command.get_args())
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    let output = command.output().with_context({
        let command_string = command_string.clone();
        move || command_string
    })?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        bail!(
            "command `{command_string}` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
