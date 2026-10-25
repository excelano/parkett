// CSV exports, from files written here.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use arrow_array::builder::{ListBuilder, StringBuilder};
use arrow_array::{ArrayRef, Int64Array, RecordBatch, StringArray};
use parkett::Table;
use parkett::export::{Exported, csv};
use parquet::arrow::ArrowWriter;

fn written(dir: &Path, rows: i64) -> Table {
    let path = dir.join("source.parquet");
    let id = Int64Array::from_iter_values(0..rows);
    let text: StringArray = (0..rows)
        .map(|i| match i {
            1 => None,
            2 => Some("a, b".to_owned()),
            3 => Some("say \"hi\"\nthen go".to_owned()),
            i => Some(format!("t{i}")),
        })
        .collect();
    let mut tags = ListBuilder::new(StringBuilder::new());
    for _ in 0..rows {
        tags.values().append_value("x");
        tags.append(true);
    }
    let batch = RecordBatch::try_from_iter(vec![
        ("id", Arc::new(id) as ArrayRef),
        ("text", Arc::new(text) as ArrayRef),
        ("tags", Arc::new(tags.finish()) as ArrayRef),
    ])
    .unwrap();
    let mut writer = ArrowWriter::try_new(File::create(&path).unwrap(), batch.schema(), None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
    Table::open(&path).unwrap()
}

/// Catches a CSV a spreadsheet would misread: a comma, quote or line break
/// inside a value left unquoted, a null written as text, a nested value not
/// JSON, or a missing header.
#[test]
fn a_csv_is_quoted_where_it_has_to_be() {
    let dir = tempfile::tempdir().unwrap();
    let table = written(dir.path(), 5);
    let to = dir.path().join("out.csv");
    let progress = AtomicU64::new(0);
    let outcome = csv(&table, &[0, 1, 2], &to, &progress, &AtomicBool::new(false)).unwrap();
    assert_eq!(outcome, Exported::Written);
    let written = std::fs::read_to_string(&to).unwrap();
    assert_eq!(
        written,
        "id,text,tags\r\n0,t0,\"[\"\"x\"\"]\"\r\n1,,\"[\"\"x\"\"]\"\r\n2,\"a, b\",\"[\"\"x\"\"]\"\r\n3,\"say \"\"hi\"\"\nthen go\",\"[\"\"x\"\"]\"\r\n4,t4,\"[\"\"x\"\"]\"\r\n"
    );
    assert_eq!(progress.load(Ordering::Relaxed), 5);
}

/// Catches the columns asked for being ignored and every column written.
#[test]
fn only_the_columns_asked_for_are_written() {
    let dir = tempfile::tempdir().unwrap();
    let table = written(dir.path(), 3);
    let to = dir.path().join("out.csv");
    csv(&table, &[0], &to, &AtomicU64::new(0), &AtomicBool::new(false)).unwrap();
    assert_eq!(std::fs::read_to_string(&to).unwrap(), "id\r\n0\r\n1\r\n2\r\n");
}

/// Catches a cancelled export leaving half a file behind, or replacing the
/// file that was already at the destination.
#[test]
fn a_cancelled_export_leaves_the_destination_alone() {
    let dir = tempfile::tempdir().unwrap();
    let table = written(dir.path(), 20_000);
    let to = dir.path().join("out.csv");
    std::fs::write(&to, "what was here").unwrap();
    let outcome = csv(&table, &[0, 1], &to, &AtomicU64::new(0), &AtomicBool::new(true)).unwrap();
    assert_eq!(outcome, Exported::Cancelled);
    assert_eq!(std::fs::read_to_string(&to).unwrap(), "what was here");
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left.len(), 2, "{left:?}");
}

/// Catches an export written over the Parquet file it reads.
#[test]
fn an_export_cannot_replace_its_source() {
    let dir = tempfile::tempdir().unwrap();
    let table = written(dir.path(), 3);
    let source = table.path().to_owned();
    let before = std::fs::read(&source).unwrap();
    assert!(csv(&table, &[0], &source, &AtomicU64::new(0), &AtomicBool::new(false)).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), before);
}
