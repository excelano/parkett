# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately, using the **Report a vulnerability** button on the Security tab of this repository. If that button is not there, or you would rather not go through GitHub, email david.anderson@excelano.com instead. I aim to respond within seven days.

Please do not open public issues for security problems.

## Supported versions

The latest release receives security fixes. Older versions are not supported.
A fix ships in a new release; update the way you installed.

## What Parkett can access

Parkett opens a Parquet file, which may have come from anywhere, and shows its
contents. It runs locally, makes no network call of any kind, has no account
and no telemetry, and can read and write only what your operating-system user
already can. It never writes to the file it opened; the one file it writes is
a CSV export, where you choose in the save dialog.

The file is decoded by the `parquet` crate from Apache Arrow's Rust
implementation, and Zstandard-compressed data by the Zstandard C library. A
malformed file is the input this application has to withstand, and a crash or
hang on one is worth reporting.

## What Parkett stores

Nothing. No recent-files list, no settings file, no cache on disk.
