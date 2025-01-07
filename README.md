# wasi-wheels

Compile Python wheels for use with WASI targets. Specifically, generate wheels that can be consumed by [componentize-py](https://github.com/bytecodealliance/componentize-py).

**This is very much a work in progress**. The goal is to get to a point where wheels are built and stored as GitHub releases, and provided through an alternate Python package registry for WASI builds until PyPi supports them natively.

Right now this tooling can:

- Setup the necessary build tooling
- Download an sdist build for a given project and version
- IN PROGRESS: build wheels for pydantic
- TODO: Upload wheels to GitHub

## Setup

Make sure you have `python3.12` and [`rustup`](https://www.rust-lang.org/learn/get-started) installed.

After cloning the repo, you can run:

```sh
cargo run -- install-build-tools
```

This will setup the latest version of [WASI SDK](https://github.com/WebAssembly/wasi-sdk) with some minor patches for compiling CPython (mainly making wasip2 look like wasi for now, until better support for the correct target is available).

It will also install and compile a [fork of Cpython](https://github.com/benbrandt/cpython/tree/3.12-wasi) that can be compiled for WASI targets with socket and dynamic linking support.

This is important, because the target of these wheels is [componentize-py](https://github.com/bytecodealliance/componentize-py) which expects support for both of these.
