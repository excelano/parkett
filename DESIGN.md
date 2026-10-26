# Parkett — design

The authority on how Parkett is built. `README.md` is the front door. Sections
are numbered because the code cites them.

## §1 A viewer for one format

Parkett opens an Apache Parquet file and shows it: every row, in a grid, at the
speed of the window rather than the size of the file, beside a panel that
describes the file itself. It reads Parquet and nothing else, and it never
writes one. A Parquet file is laid down once by whatever produced it, its
columns compressed and encoded in row groups with an index at the end, and a
change to one cell moves every byte after it; a tool that rewrote one would
hand back a different file under the same name. The one thing Parkett writes is
an export (§7), which is a new file in another format.

It is one binary over egui and eframe, packaged for apt, the Microsoft Store
and the Mac App Store the way the fleet's other egui applications are.
The library, `src/lib.rs` and its modules, reads files and knows nothing of a window, and is
`forbid(unsafe_code)`; `src/main.rs` is the window.

## §2 What the build depends on

The file is read by the `parquet` crate from `arrow-rs`, with its Arrow reader
and its default codecs. Snappy, gzip, LZ4 and Brotli are pure Rust. Zstandard
is the C library, compiled by `zstd-sys` and linked statically, and it is the
one C dependency. What it costs is a C compiler on each machine that builds,
which each already has, and a rebuild for a Zstandard security fix.

Two checks hold the build to that, because neither alone does. The artefact's
dynamic dependencies:

    objdump -p "$target/release/parkett" | grep NEEDED

answer `libgcc_s`, `libm` and `libc` and nothing else, so nothing is loaded
from the system at run time. That list cannot see a library compiled in
statically, so the second check names the packages whose build scripts compiled
an object file, scoped to this tree because the target directory may be shared:

    find "$target/release/build" -name '*.o' \
      | sed -E 's|.*/build/(.*)-[0-9a-f]{16}/.*|\1|' | sort -u \
      | grep -xFf <(cargo tree -e normal,build --prefix none --target all | awk '{print $1}' | sort -u)

and the answer is `zstd-sys` alone. A second name there is a second C library,
and is decided rather than absorbed. `$target` is what `cargo metadata` reports
as `target_directory`.

A file whose codec the build lacks, which is LZO alone, opens, shows its schema
and metadata, and says in the grid which codec it would have needed.

## §3 Reading

A file is opened by path and read from the disk as the grid asks for it, never
in full. Opening reads the footer alone: the schema, the row count, and where
each row group and column chunk lies. That is enough to draw the panel (§6), to
size the grid, and to say how many rows there are before a single value has
been decoded.

Values are read in blocks. A block is a run of rows within one row group, in
the columns the viewport covers, and only those columns are decoded. What a
block costs is mostly fixed: most writers dictionary-encode a column, and the
dictionary page of each column chunk is decompressed and decoded before any of
its rows can be, however few are wanted. So a block is many screens tall, tens
of thousands of rows, and the cost is paid once per stretch of the file rather
than once per scroll. When the file carries a page index, the reader skips the
pages outside the block; many writers do not write one, and nothing depends on
it.

Blocks are kept in a cache bounded by memory rather than by count, and a block
falls out of it in the order it was last drawn. Reading happens off the
interface's thread; a cell whose block has not arrived is drawn empty and the
grid repaints when it lands. A block that fails to decode leaves the rest of the
file readable: its cells say why, and the footer, the panel and every other
block are unaffected.

## §4 The grid

The grid is painted the way `xods` paints its sheet, with the pattern carried
into this repository rather than the code depended on. Cells are
painted for the viewport and no others, columns are measured once into a table
of where each begins, a pointer finds a column by searching that table, and a
column is resized by dragging its header's edge.

Rows differ from a sheet's in two ways. They are all one height, so a row's
position is its index times that height and needs no table. And there can be
more of them than a scroll offset can address: egui measures in `f32`, which
stops placing rows to the pixel somewhere past a million of them. So the grid
scrolls by row index. The scroll position is a `u64` row and a fraction of one,
the bar beside the grid is Parkett's own and maps its length onto the row count,
and egui is only ever asked to place the rows in view, at offsets of a screen
or less.

Columns are as wide as the wider of their header and their first values, up to
a limit, and a person widens or narrows one by dragging its header's edge; a
value wider than its column ends in an ellipsis, and a double-click on the
header's edge fits the column to its values in view. Numbers
are set right and everything else left. Column headers carry the column's name
and, beneath it, its type in the words §5 uses. Row headers carry the row's
number from 1, as a spreadsheet does.

A selection is a rectangle of cells: one cell by a click or the arrow keys, Page
Up and Down, Home and End, with Ctrl reaching the edges of the file; a range by
Shift with any of those or by a drag, which scrolls the grid when it passes the
edge; a column by its header, a row by its number and the file by the corner
between them, or Ctrl+A. Escape shrinks a range back to its cursor cell, the
one the keys move and the pane beneath the grid shows, and on one cell clears
the selection.

## §5 Values

A value is shown in the form its logical type gives it, not its physical one,
and Parkett formats each one itself from the Arrow array rather than through
`arrow-cast`. A timestamp is a date and time to the precision its unit records, so a
column of them lines up: one adjusted to UTC is shown in UTC and marked as
such, one in a named zone is shown in that zone with its offset, from the time
zone database `chrono-tz` carries, and one with no zone is shown as written. The
deprecated 96-bit timestamps are read as timestamps rather than as numbers. A decimal keeps its scale. A date is a
date. A string is text, and binary that is not marked as text is shown as
hexadecimal up to the cell's width.

Null is drawn as a faint mark rather than as empty text, so that a null and an empty
string can be told apart at a glance.

A nested value, a list, a struct or a map, is shown in the cell as compact JSON
cut to the cell's width. The selected cell, of any type, is shown whole beneath the
grid, with a nested value indented, where its text can be selected. Columns are not flattened into one per leaf; the grid
has the columns the schema has at its top level.

## §6 The file panel

A side panel, opened from the View menu and closed until it is, describes the
file from its footer and needs no values decoded.
It shows the schema as a tree with each field's physical type, logical type and
whether it may be null; the row count, the number of row groups and their
sizes; the writer the file names; the key and value metadata the producer
stored; and, for each column, its codec, its encodings, its compressed and
uncompressed size, and the minimum, maximum and null count its statistics
record: the null count across the file, and the minimum and maximum per row
group, since each row group's are compared in the column's own sort order and
a file-wide answer would need that order applied across them. A minimum and
maximum written by the writers that compared byte arrays as signed are not
shown.

## §7 Copy and export

Copy hands one cell over as its text and a range as tab-separated rows, which a
spreadsheet pastes as cells: a null is an empty field, and a field holding a
tab, a line break or a quote is quoted. A copy decodes what it needs at once, so
it is bounded: a range of more than a million cells is refused with a word in
the pane beneath the grid, since a range that large is an export.

Export writes every row of the file, or of the columns a selection spans, as
CSV with a header row: comma-separated, quoted where a value holds a comma, a
quote or a line break, lines ended by CRLF, as RFC 4180 has it. Each value is
in the form §5 shows it but for timestamps, which are ISO 8601 with no zone
designator, and one with a zone is written as its UTC instant: a spreadsheet
takes the grid's form, and any designator, for text. The zone stays in the
Parquet file's schema, and a program reading the CSV takes the times as UTC. A null is an empty field, a
nested value is JSON and binary is hexadecimal, the conventions this2that's
`parquet2csv` also keeps. The destination is chosen in a save dialog and cannot be the file being
read. The CSV is written beside it under a hidden name and moved into place
only once it is whole, so an export that fails or is cancelled leaves whatever
was there as it was. It runs on its own thread, with its progress in rows and a
Cancel beneath the grid, and Comma is the application that opens what it
writes.

## §8 What it does not do

Parkett does not edit, write Parquet, sort, filter or run queries. Sorting and
filtering a file that does not fit in memory needs a query engine, and the
engines that do it in Rust are many times the size of everything else here;
DuckDB answers those questions from the command line already. It does not open
a directory of Parquet files as one table, read from object storage, or open
Arrow, ORC or Avro files.

## §9 Language

Every sentence Parkett shows is looked up in a `.po` catalogue compiled into
the binary and read by `potext`, so the same mechanism works in a `.deb`, an
MSIX package and a crate with nothing installed beside it. The language is the
one the platform names, or `POTEXT_LANG` when it is set, and a language with no
catalogue is English. A message whose English has changed since it was
translated is marked fuzzy by `msgmerge`, and `potext` will not show a fuzzy
entry, so a stale translation falls back to English rather than saying
something the window no longer means.

What Parkett shows of the file is not translated: values, the `null` mark, the
type names of §5, codec and encoding names, and the reader's own messages when
a file cannot be read. Counts and sizes the window writes are grouped and
pointed the way the language writes them, through two messages keyed by
context, so German shows 5.000.000 and 1,5 MB.

`po/update-po.sh` re-reads the sources into `po/parkett.pot` and merges it into
each catalogue; it runs after any change to a sentence a person reads.
`po/pseudo.sh` writes `po/en-x-pseudo.po`, every message accented, bracketed
and 40% longer, which debug builds alone compile in: a string still plain
English never reached the catalogue, and a cut-off label was laid out to the
width of English.
