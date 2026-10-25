# macOS

The Mac App Store bundle and package, and the store screenshots.

| File | What it is |
|---|---|
| `build-app.sh` | Assembles `dist/Parkett.app`; `--universal` joins the two architectures, `--sign ID` signs it with the sandbox entitlement, `--store PROFILE` builds the `.pkg` the Mac App Store takes |
| `Info.plist.in` | The bundle's property list, filled in by `build-app.sh` |
| `Parkett.entitlements` | The App Sandbox and read-write access to the files a person chooses, and nothing else |
| `check-install.sh` | Asks an installed Parkett what Launch Services and the sandbox make of it |
| `shots.sh` | Parkett's screenshot recipes; `take-shots.sh`, `screenshot.sh` and the Swift helpers are the fleet's driver |

## Building

    export MACOSX_DEPLOYMENT_TARGET=11.0
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    ./packaging/macos/build-app.sh --universal --sign "Apple Development: …"
    open -a dist/Parkett.app packaging/demo/observations.parquet

The floor is `LSMinimumSystemVersion` in `Info.plist.in`, and the build checks
both slices against it. `security find-identity -v -p codesigning` lists the
identities the machine holds: an Apple Development identity is enough to try
the sandbox, which does nothing until the entitlement is inside a signature,
and a Store upload is signed with Apple Distribution and its package with Mac
Installer Distribution, which `--store` chooses itself.

## What the code does for macOS

macOS hands a double-clicked file to an application as an Apple Event rather
than as an argument, and `src/opened_document.rs` receives it; it is the one
module in Parkett that writes `unsafe`. Under the sandbox an export cannot be
written beside its destination under another name, so `src/staging.rs` writes a
new destination in place and replaces an existing one from the directory macOS
provides for that, on the same volume. The window is handed an empty icon so
that eframe does not put its own logo in the Dock in place of the bundle's.

## The icon and the type

The application icon is the square drawing, `packaging/icons/parkett-square.svg`,
which macOS masks to its own shape; the document icon is
`packaging/linux/icons/application-vnd.apache.parquet.svg`. Both are rendered
into `.icns` files at build time. Parquet is not Parkett's format, so the
bundle imports its type, `org.apache.parquet`, rather than exporting it, and
claims it as a viewer.

## Screenshots

`shots-macstore` takes the set on the Mac from `shots.sh`, light and dark, in
each listed language. Every step is a key, so the recipes carry no coordinates.
