// A range of cells as tab-separated text, from files written here.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs::File;
use std::sync::Arc;

use arrow_array::{ArrayRef, Int64Array, RecordBatch, StringArray};
use parkett::copy::{COPY_CELLS, tab_separated};
use parkett::{BLOCK_ROWS, Blocks, Table};
use parquet::arrow::ArrowWriter;

const ROWS: i64 = BLOCK_ROWS as i64 + 10;

fn opened() -> (tempfile::TempDir, Arc<Table>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cells.parquet");
    let id = Int64Array::from_iter_values(0..ROWS);
    let text: StringArray = (0..ROWS)
        .map(|i| match i {
            1 => None,
            2 => Some("a\tb".to_owned()),
            3 => Some("say \"hi\"".to_owned()),
            i => Some(format!("t{i}")),
        })
        .collect();
    let batch = RecordBatch::try_from_iter(vec![("id", Arc::new(id) as ArrayRef), ("text", Arc::new(text) as ArrayRef)])
        .unwrap();
    let mut writer = ArrowWriter::try_new(File::create(&path).unwrap(), batch.schema(), None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
    let table = Arc::new(Table::open(&path).unwrap());
    (dir, table)
}

/// Catches a null copied as the word null, and a tab or quote inside a value
/// splitting it into two fields when pasted.
#[test]
fn nulls_are_empty_and_awkward_values_are_quoted() {
    let (_dir, table) = opened();
    let mut blocks = Blocks::new(&table, || {});
    let text = tab_separated(&table, &mut blocks, 0..=4, 0..=1).unwrap();
    assert_eq!(text, "0\tt0\n1\t\n2\t\"a\tb\"\n3\t\"say \"\"hi\"\"\"\n4\tt4\n");
}

/// Catches rows lost or repeated where a range crosses from one block into the
/// next, which a copy has to decode without waiting for the workers.
#[test]
fn a_range_across_blocks_is_whole() {
    let (_dir, table) = opened();
    let mut blocks = Blocks::new(&table, || {});
    let (start, end) = (BLOCK_ROWS - 2, BLOCK_ROWS + 1);
    let text = tab_separated(&table, &mut blocks, start..=end, 0..=0).unwrap();
    let ids: Vec<u64> = text.lines().map(|l| l.parse().unwrap()).collect();
    assert_eq!(ids, (start..=end).collect::<Vec<_>>());
}

/// Catches a copy of a whole large file attempted, which would hold the window
/// while it decoded everything.
#[test]
fn a_copy_larger_than_the_limit_is_refused() {
    let (_dir, table) = opened();
    let mut blocks = Blocks::new(&table, || {});
    let rows = COPY_CELLS / 2 + 1;
    assert!(tab_separated(&table, &mut blocks, 0..=rows, 0..=1).is_err());
}
