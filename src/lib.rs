//! Tooling to generate Python wheels usable in WASI contexts and consumable as a Python registry.

use std::iter;

use anyhow::{bail, Context};
use tokio::process::Command;

mod build;
mod python_registry;

pub use build::{install_build_tools, REPO_DIR};
pub use python_registry::download_sdist;

/// Run a given command with common error handling behavior
///
/// # Errors
///
/// Returns error if the command fails for any reason.
pub async fn run(command: &mut Command) -> anyhow::Result<Vec<u8>> {
    let command_string = iter::once(command.as_std().get_program())
        .chain(command.as_std().get_args())
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    let output = command.output().await.with_context({
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
