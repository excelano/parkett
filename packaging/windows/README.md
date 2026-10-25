# Windows

The Microsoft Store package, the per-user install for a source checkout, and
the Store screenshots.

| File | What it is |
|---|---|
| `AppxManifest.xml.in` | The MSIX manifest, filled in by `build-msix.ps1` |
| `identity.psd1` | The package identity Partner Center assigned: name, family name, Store id |
| `build-msix.ps1` | Builds the MSIX from the release executable; `-SelfSign` to install it here, `-Certify` to run the certification kit |
| `check-imports.ps1` | Refuses an executable that imports a DLL Windows does not ship |
| `parkett.manifest` | The application manifest `build.rs` embeds, declaring DPI awareness |
| `parkett.ico`, `parquet.ico` | The application's icon and the Parquet document's, for the window and the per-user install |
| `assets/`, `listing/` | The images the manifest names, and the Store listing logo |
| `make-ico/` | Renders the two `.ico` files, `assets/`, `listing/` and the PNGs in `packaging/icons` |
| `install.ps1`, `uninstall.ps1` | The per-user install under `%LOCALAPPDATA%` and its removal |
| `shots.ps1` | Parkett's screenshot recipes; `take-shots.ps1` and `screenshot.ps1` are the fleet's driver |

## Building

    cargo build --release
    powershell -ExecutionPolicy Bypass -File packaging\windows\check-imports.ps1
    powershell -ExecutionPolicy Bypass -File packaging\windows\build-msix.ps1 -SelfSign
    Add-AppxPackage dist\Parkett-<version>-x64.msix

`build-msix.ps1` reads the account's publisher string from `STORE_PUBLISHER`.
`.cargo/config.toml` links the Visual C++ runtime statically, so the executable
imports nothing Windows does not carry, and `check-imports.ps1` is what holds it
to that; the package refuses to build from an executable that fails it.

## The icon

Windows takes the square drawing, `packaging/icons/parkett-square.svg`, and
plates or rounds it itself. `make-ico` writes every size from it and from the
document drawing in `packaging/linux/icons`; its outputs are committed and are
not edited by hand. Rerun it after changing either drawing:

    cd packaging\windows\make-ico
    cargo run --release

## The association

The package and the per-user install both register `.parquet` with the media
type `application/vnd.apache.parquet`, a name and the document icon, since
Windows registers nothing for the extension. A packaged association adds Parkett
under Open With and never takes the extension's default; the per-user install
does take it, and names itself "Parkett (user install)" in Settings so the two
can be told apart. Uninstall one before installing the other.

## Certification

The certification kit's one finding is the optional Blocked executables test:
the standard library's process spawning, linked through `webbrowser` under
egui-winit, and `ShellExecuteW`, which the export's Open button uses to hand a
CSV to Windows. `build-msix.ps1 -Certify` names what it expects and refuses
anything else.

## Screenshots

`shots-winstore` takes the set on the Windows machine from `shots.ps1`. Every
step is a key, so the recipes carry no coordinates and the German set is the
same recipe in a German window. The file shown is `packaging/demo/observations.parquet`,
which `cargo run --release --example demo -- packaging/demo/observations.parquet`
writes.
