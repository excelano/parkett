#!/bin/sh
# Re-read every translatable string out of the source and bring each catalogue
# up to date with it. Run after changing any sentence a person reads.
#
# xgettext has no Rust, so the sources are read as C: write every message as one
# string literal (C would glue adjacent literals into a msgid nothing looks up),
# and never a raw string. The warnings it prints about lifetimes are noise;
# pseudo.sh is how a string that never reached the catalogue is found.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$here/.."

domain=parkett
pot="po/$domain.pot"
sources=$(find src -name '*.rs' | sort)

xgettext \
    --language=C \
    --from-code=UTF-8 \
    --keyword \
    --keyword=t \
    --keyword=tc:1c,2 \
    --keyword=tn:1,2 \
    --add-comments=Translators: \
    --sort-by-file \
    --package-name="$domain" \
    --msgid-bugs-address=https://github.com/excelano/parkett/issues \
    --output="$pot" \
    $sources

sed -i "s/^\"Project-Id-Version: $domain VERSION/\"Project-Id-Version: $domain/" "$pot"
# msginit would otherwise write charset=ASCII into a new catalogue, and the
# first German word would then be refused.
sed -i 's/charset=CHARSET/charset=UTF-8/' "$pot"

# msgmerge marks a reworded message fuzzy, and potext will not show a fuzzy
# entry, so a stale translation falls back to English rather than being wrong.
for catalogue in po/*.po; do
    [ -e "$catalogue" ] || continue
    [ "$catalogue" = po/en-x-pseudo.po ] && continue
    msgmerge --quiet --update --backup=none --previous "$catalogue" "$pot"
    msgfmt --check --output-file=/dev/null "$catalogue"
    printf '%s: ' "$catalogue"
    msgfmt --statistics --output-file=/dev/null "$catalogue" 2>&1
done
