# Submission notes

What a store submission needs from a person and no file supplies. The listing
text is `store-listing.toml` and what a release changed is `release-notes.toml`;
the release apps push both. A field edited here reaches nobody.

## Capability justification

Partner Center asks once, when `runFullTrust` is first declared, and keeps the
answer against the product. 500 characters, newlines counted.

> Parkett is a full-trust Win32 desktop application packaged as MSIX, and it
> needs this capability to run at all. It opens the Parquet files a person
> chooses and writes a CSV only where they choose in the save dialog. It
> makes no network connection and uses no device.

## The answers a form asks

    Category                  Developer tools (both stores)
    Copyright and trademark   Excelano LLC
    Additional licence terms  blank; the application is MIT and the
                              repository carries the licence
    Privacy policy            https://excelano.com/legal/#parkett
    Age rating                no objectionable content of any kind
    App Privacy (Apple)       Data Not Collected
    Price                     free

## Screenshots

The same three frames on both stores, taken by `shots-winstore` and
`shots-macstore` from the keyboard recipes in `windows/shots.ps1` and
`macos/shots.sh`, over `demo/observations.parquet`: the grid with a list
selected and shown whole beneath it, the file panel beside the grid, and a
range selected as it would be copied. The Mac set repeats the first two in
dark.
