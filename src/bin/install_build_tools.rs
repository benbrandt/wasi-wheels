//! Prep Cpython and WASI SDK tooling

use wasi_wheels::download_and_compile_cpython;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    download_and_compile_cpython().await
}
