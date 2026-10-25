# Parkett

A desktop viewer for Apache Parquet files.

Parkett opens a `.parquet` file as a grid that scrolls through millions of rows
at the speed of the window, reading only the stretch in view, and beside it, on
request, what the file says about itself, from its schema and row groups down
to each column's codec, encodings, sizes and statistics. It reads Parquet and never writes it, because a Parquet file is
written once by whatever produced it and a viewer that changed one would hand
back a different file under the same name.

It is a native application rather than a browser page or an Electron shell, it
opens a file without reading all of it, and it asks for nothing but the file.
For questions a viewer cannot answer, sorting a file larger than memory or
joining it to another, DuckDB's command line reads Parquet directly.

## Build

```
cargo build --release
```

Building needs a Rust toolchain and a C compiler, for the Zstandard library.
`Cargo.toml` names the Rust version it needs.

## Use

```
parkett orders.parquet
```

or open the application and choose a file with File, Open.

`DESIGN.md` is the authority on how Parkett is built.

## Licence

MIT. See `LICENSE`.
