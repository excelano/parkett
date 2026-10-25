# Packaging

How Parkett is built into what each platform installs. Releases are the apps in
`excelano/shipping`, run from the repository root with `ship.toml`; this
directory is what they build from.

| Path | What it is |
|---|---|
| `version.sh` | The version in the spellings the platforms want; the only reader of `Cargo.toml`'s |
| `icons/` | The icon square, for the stores, and rounded, for the Linux desktop, with PNG exports for submission forms |
| `linux/` | The desktop entry, the rounded icon, the Parquet document icon and media type, the per-user install, and `check-libraries.sh` |
| `debian/` | `build-deb.sh`, the control file, the changelog, the copyright file and the manual page |
| `windows/` | The MSIX and its checks; its README says more |
| `macos/` | The bundle and the Store package; its README says more |
| `demo/` | The file the store screenshots show, written by `examples/demo.rs` |
| `store-listing.toml`, `release-notes.toml` | What the stores show, and what a release tells them changed |
| `submission-notes.md` | What a submission form asks that no file answers |
| `privacy-entry.html` | The section for the privacy page at excelano.com/legal |

## The icon

`icons/parkett-square.svg` is a basketweave parquet floor, edge to edge, and is
what the Microsoft Store, Windows and the Mac App Store take: each applies its
own shape. `icons/parkett-rounded.svg` is the same drawing clipped to a rounded
square, which `linux/icons/parkett.svg` copies, since the Linux desktop draws an
icon as it is given. `linux/icons/application-vnd.apache.parquet.svg` is the
document icon every platform gives a `.parquet` file. `windows/make-ico`
renders the Windows sizes and the PNG exports; `macos/build-app.sh` renders the
`.icns` files at build time.

## The media type

Parquet's media type is `application/vnd.apache.parquet`. shared-mime-info,
Windows and macOS declare nothing for it, so each package declares it: a
shared-mime-info entry in the `.deb`, a file-type association in the MSIX, and
an imported type, `org.apache.parquet`, in the bundle.
