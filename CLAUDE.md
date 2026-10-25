# CLAUDE.md

A desktop viewer for Apache Parquet files, on egui and eframe. `DESIGN.md` is the
authority on the application; read the section a change touches before making it.
`src/lib.rs` reads files and knows nothing of a window; `src/main.rs` is the window.

## Commands

    cargo build --release
    cargo test
    cargo clippy --all-targets        # must be silent

There is no Windows cross-check from Linux: `zstd-sys` compiles C for the target
and needs the MSVC tools, so a Windows check runs on the Windows machine.

The target directory may not be `./target`: `[build] target-dir` moves it, and
`cargo metadata` reports where it is.

## Rules

Parkett never writes Parquet and never writes to the file it opened (`DESIGN.md` §1).
Zstandard is the one C library, and the check in `DESIGN.md` §2 is run after any
dependency change: a second package compiling an object file is David's decision.
Tests write the files they read with the `parquet` crate rather than committing
third-party test files, and each test's doc comment says what defect it catches.
A `cargo update` or an `eframe` bump off winit 0.30.13 stops `[patch.crates-io]`
applying, which is the moment to check whether that winit release carries the
`private-apple-apis` gate.
