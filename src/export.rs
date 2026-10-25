//! Every row of a file, or of some of its columns, written as CSV. DESIGN.md §7.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::i18n::t;
use crate::staging::Staging;
use crate::{Table, values};

const BATCH_ROWS: usize = 8192;

#[derive(Debug, PartialEq, Eq)]
pub enum Exported {
    Written,
    Cancelled,
}

/// Write `columns` of every row to `to` as CSV with a header row, counting
/// rows into `progress` and stopping when `cancel` is set.
///
/// The CSV is written whole before it takes `to`'s place, so a cancelled or
/// failed export leaves whatever was at `to` as it was.
///
/// # Errors
///
/// When `to` is the file being read, or the file cannot be read or the CSV
/// written.
pub fn csv(table: &Table, columns: &[usize], to: &Path, progress: &AtomicU64, cancel: &AtomicBool) -> Result<Exported, String> {
    if same_file(table.path(), to) {
        return Err(t("An export cannot replace the file it is read from.").to_owned());
    }
    let staging = Staging::for_destination(to).map_err(|e| e.to_string())?;
    match write(table, columns, staging.path(), progress, cancel)? {
        Exported::Written => staging.finish().map(|()| Exported::Written).map_err(|e| e.to_string()),
        Exported::Cancelled => Ok(Exported::Cancelled),
    }
}

fn write(table: &Table, columns: &[usize], to: &Path, progress: &AtomicU64, cancel: &AtomicBool) -> Result<Exported, String> {
    let mut out = BufWriter::new(File::create(to).map_err(|e| e.to_string())?);
    let mut line = String::new();
    for (i, &column) in columns.iter().enumerate() {
        if i > 0 {
            line.push(',');
        }
        field(&mut line, &table.names[column]);
    }
    line.push_str("\r\n");
    for batch in table.batches(columns, BATCH_ROWS)? {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Exported::Cancelled);
        }
        let batch = batch.map_err(|e| e.to_string())?;
        for row in 0..batch.num_rows() {
            for (i, array) in batch.columns().iter().enumerate() {
                if i > 0 {
                    line.push(',');
                }
                if let Some(text) = values::exported(array.as_ref(), row) {
                    field(&mut line, &text);
                }
            }
            line.push_str("\r\n");
        }
        out.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        line.clear();
        progress.fetch_add(batch.num_rows() as u64, Ordering::Relaxed);
    }
    out.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(Exported::Written)
}

/// A field quoted when it holds a comma, a quote or a line break. RFC 4180.
fn field(out: &mut String, text: &str) {
    if text.contains([',', '"', '\n', '\r']) {
        out.push('"');
        out.push_str(&text.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(text);
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}
