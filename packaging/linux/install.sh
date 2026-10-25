#!/bin/sh
# Install Parkett for one person from a source checkout: the executable, the
# desktop entry, the icons and the Parquet media type, under PREFIX.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/../.." && pwd)
prefix="${HOME}/.local"
no_binary=""

usage() {
    cat <<'USAGE'
usage: install.sh [--prefix DIR] [--no-binary]

  --prefix DIR  where to install (default: ~/.local)
  --no-binary   install the desktop integration only, for a parkett on PATH

Without --no-binary, parkett is taken from the Cargo target directory,
release before debug, and installed as PREFIX/bin/parkett.
USAGE
}

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) prefix="${2:?--prefix needs a directory}"; shift 2 ;;
        --no-binary) no_binary=yes; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "install.sh: unknown argument $1" >&2; usage >&2; exit 2 ;;
    esac
done

mkdir -p \
    "${prefix}/share/applications" \
    "${prefix}/share/icons/hicolor/scalable/apps" \
    "${prefix}/share/icons/hicolor/scalable/mimetypes" \
    "${prefix}/share/mime/packages"
install -m 0644 "${here}/parkett.desktop" "${prefix}/share/applications/parkett.desktop"
install -m 0644 "${here}/icons/parkett.svg" "${prefix}/share/icons/hicolor/scalable/apps/parkett.svg"
install -m 0644 "${here}/icons/application-vnd.apache.parquet.svg" \
    "${prefix}/share/icons/hicolor/scalable/mimetypes/application-vnd.apache.parquet.svg"
install -m 0644 "${here}/mime/parquet.xml" "${prefix}/share/mime/packages/parkett-parquet.xml"

if [ -z "$no_binary" ]; then
    target_dir=$(cd "$root" && cargo metadata --format-version 1 --no-deps |
        sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
    found=""
    for candidate in "${target_dir}/release/parkett" "${target_dir}/debug/parkett"; do
        [ -x "$candidate" ] && { found=$candidate; break; }
    done
    if [ -n "$found" ]; then
        mkdir -p "${prefix}/bin"
        install -m 0755 "$found" "${prefix}/bin/parkett"
        echo "installed ${prefix}/bin/parkett from ${found}"
    else
        echo "no parkett under ${target_dir}; it must be on PATH for the entry to work"
    fi
fi

[ -x "$(command -v update-mime-database || true)" ] && update-mime-database "${prefix}/share/mime" || true
[ -x "$(command -v update-desktop-database || true)" ] &&
    update-desktop-database "${prefix}/share/applications" || true
[ -x "$(command -v gtk-update-icon-cache || true)" ] &&
    gtk-update-icon-cache -q -t -f "${prefix}/share/icons/hicolor" || true

echo "installed the Parkett desktop entry, icons and media type under ${prefix}"
echo "check with:"
echo "  gio mime application/vnd.apache.parquet    # lists parkett.desktop"
