#!/bin/sh
# Write the pseudolocale: every message accented, 40% longer and bracketed.
# Run a debug build with POTEXT_LANG=en-x-pseudo and three defects show on
# sight: a string still in English never went through t, a sentence without
# brackets is one the catalogue never saw, and a cut-off label was laid out to
# the width of English. The tag is BCP-47 private use, because potext drops a
# gettext @modifier. Debug builds alone compile it in.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$here/.."

pot=po/parkett.pot
out=po/en-x-pseudo.po

# {placeholders} are left alone, since they are filled by name. The charset is
# forced to UTF-8 before the filter, or msgfilter drops every accent it adds.
msgen "$pot" |
    sed 's/charset=[A-Za-z0-9_-]*/charset=UTF-8/' |
    msgfilter --keep-header --output-file="$out" awk '
{
    line = $0
    out = ""
    while (match(line, /\{[^}]*\}/)) {
        out = out accent(substr(line, 1, RSTART - 1)) substr(line, RSTART, RLENGTH)
        line = substr(line, RSTART + RLENGTH)
    }
    out = out accent(line)
    pad = ""
    for (i = 0; i < int(length($0) * 0.4) + 1; i++) pad = pad "·"
    printf "%s", "[" out " " pad "]"
}
function accent(s,   n, i, c, r) {
    n = ""
    for (i = 1; i <= length(s); i++) {
        c = substr(s, i, 1)
        r = c
        if (c == "a") r = "ä"; else if (c == "A") r = "Å"
        else if (c == "e") r = "é"; else if (c == "E") r = "É"
        else if (c == "i") r = "í"; else if (c == "I") r = "Î"
        else if (c == "o") r = "ö"; else if (c == "O") r = "Ö"
        else if (c == "u") r = "ü"; else if (c == "U") r = "Ü"
        else if (c == "n") r = "ñ"; else if (c == "s") r = "š"
        n = n r
    }
    return n
}'

sed -i 's/charset=CHARSET/charset=UTF-8/' "$out"
sed -i 's/^"Language: \\n"/"Language: en-x-pseudo\\n"/' "$out"
sed -i 's/^"Language-Team: LANGUAGE <LL@li.org>\\n"/"Language-Team: none\\n"/' "$out"
sed -i 's/^"Plural-Forms: nplurals=INTEGER; plural=EXPRESSION;\\n"/"Plural-Forms: nplurals=2; plural=(n != 1);\\n"/' "$out"

msgfmt --check --output-file=/dev/null "$out"
printf '%s: ' "$out"
msgfmt --statistics --output-file=/dev/null "$out" 2>&1
