# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately through GitHub Security
Advisories at https://github.com/excelano/parkett/security/advisories/new.
If you would rather not use GitHub, email david.anderson@excelano.com instead. I
aim to respond within seven days.

Please do not open public issues for security problems.

## Supported versions

The latest 0.x release receives security fixes. Older versions are not
supported.

## What this application is

Parkett opens a Parquet file, which may have come from anywhere, and shows its
contents. It runs locally, makes no network call of any kind, has no account
and no telemetry, and can read and write only what your operating-system user
already can. It never writes to the file it opened.

The file is decoded by the `parquet` crate from Apache Arrow's Rust
implementation, and Zstandard-compressed data by the Zstandard C library. A
malformed file is the input this application has to withstand, and a crash or
hang on one is worth reporting.

## What it stores

Nothing. No recent-files list, no settings file, no cache on disk.

## Verifying releases

Every release lists a SHA-256 for each artefact. Verify a download before
running it.
