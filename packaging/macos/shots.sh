#!/bin/sh
# The Mac App Store screenshots, as recipes. screenshot.sh and take-shots.sh
# beside this are the fleet's generated driver; this file is Parkett's: which
# file, and what the keys do before each shutter.
#
#     ./packaging/macos/shots.sh --app dist/Parkett.app
#     ./packaging/macos/shots.sh --app dist/Parkett.app --lang de
#     ./packaging/macos/shots.sh --app dist/Parkett.app --reference
#
# Every step is a key: the grid takes the arrows, Shift with them and Cmd+I
# once the window is in front, so the recipes carry no coordinates and the
# German set is the English recipe in a German window. The file opens through
# Launch Services, as a double-click would, so the set also shows the Apple
# Event handler working. packaging/windows/shots.ps1 is the same three frames.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/../.." && pwd)

# One of the sizes App Store Connect accepts, and the largest reachable
# without a Retina display.
WIDTH=1440
HEIGHT=900

# Two steps of egui's own zoom, so the text survives the store's thumbnails.
EVERY_SHOT="--key cmd+plus --key cmd+plus"

for_language() {
    case "$1" in
        en|en-US|en-us|de|de-DE|de-de) ;;
        *) echo "shots.sh: no set is written for $1" >&2; exit 2 ;;
    esac
    document="${root}/packaging/demo/observations.parquet"
    staged_name="observations.parquet"
}

repeat() {
    n=$2
    while [ "$n" -gt 0 ]; do
        printf ' --key %s' "$1"
        n=$((n - 1))
    done
}

# shellcheck disable=SC2046
shots() {
    appearance light
    shot 01-grid $(repeat down 4) $(repeat right 6) --settle 2
    shot 02-details $(repeat down 2) $(repeat right 3) --key cmd+i --settle 2
    shot 03-range $(repeat down 3) $(repeat shift+down 9) $(repeat shift+right 4) --settle 2

    appearance dark
    shot 04-grid-dark $(repeat down 4) $(repeat right 6) --settle 2
    shot 05-details-dark $(repeat down 2) $(repeat right 3) --key cmd+i --settle 2
}

. "${here}/take-shots.sh"

# shellcheck disable=SC2068
take_shots $@
