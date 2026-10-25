//! A range of cells as tab-separated text, for the clipboard. DESIGN.md §7.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::ops::RangeInclusive;

use crate::{Blocks, Table, values};

/// The most cells one copy carries; a larger range is an export.
pub const COPY_CELLS: u64 = 1_000_000;

/// The cells of `rows` by `columns`, one line per row and a tab between
/// cells, decoding any block that has not arrived. A null is an empty field,
/// and a field holding a tab, a line break or a quote is quoted.
///
/// # Errors
///
/// When the range holds more than [`COPY_CELLS`] cells, or a block cannot be
/// decoded.
pub fn tab_separated(
    table: &Table,
    blocks: &mut Blocks,
    rows: RangeInclusive<u64>,
    columns: RangeInclusive<usize>,
) -> Result<String, String> {
    let cells = (rows.end() - rows.start() + 1) * (columns.end() - columns.start() + 1) as u64;
    if cells > COPY_CELLS {
        return Err(format!("{cells} cells is more than one copy carries; select fewer."));
    }
    let mut out = String::new();
    for row in rows {
        for column in columns.clone() {
            if column != *columns.start() {
                out.push('\t');
            }
            let (id, offset) = table.block_of(column, row);
            let array = blocks.array(table, id)?;
            if let Some(text) = values::text(array.as_ref(), offset) {
                field(&mut out, &text);
            }
        }
        out.push('\n');
    }
    Ok(out)
}

fn field(out: &mut String, text: &str) {
    if text.contains(['\t', '\n', '\r', '"']) {
        out.push('"');
        out.push_str(&text.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(text);
    }
}
