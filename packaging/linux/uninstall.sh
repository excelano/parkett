#!/bin/sh
# Remove what install.sh put in place, and rebuild the caches that indexed it.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

prefix="${HOME}/.local"
keep_binary=""

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) prefix="${2:?--prefix needs a directory}"; shift 2 ;;
        --keep-binary) keep_binary=yes; shift ;;
        -h|--help)
            echo "usage: uninstall.sh [--prefix DIR] [--keep-binary]"; exit 0 ;;
        *) echo "uninstall.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

rm -f \
    "${prefix}/share/applications/parkett.desktop" \
    "${prefix}/share/icons/hicolor/scalable/apps/parkett.svg" \
    "${prefix}/share/icons/hicolor/scalable/mimetypes/application-vnd.apache.parquet.svg" \
    "${prefix}/share/mime/packages/parkett-parquet.xml"

[ -n "$keep_binary" ] || rm -f "${prefix}/bin/parkett"

[ -x "$(command -v update-mime-database || true)" ] && update-mime-database "${prefix}/share/mime" || true
[ -x "$(command -v update-desktop-database || true)" ] &&
    update-desktop-database "${prefix}/share/applications" || true
[ -x "$(command -v gtk-update-icon-cache || true)" ] &&
    gtk-update-icon-cache -q -t -f "${prefix}/share/icons/hicolor" || true

echo "removed the Parkett desktop integration from ${prefix}"
