// Reading values in blocks, from files written here.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow_array::cast::AsArray;
use arrow_array::types::Int64Type;
use arrow_array::{ArrayRef, Int64Array, RecordBatch, StringArray};
use parkett::{BLOCK_ROWS, Blocks, Table};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

/// More rows than a block holds, in row groups that do not divide by a block.
const ROWS: i64 = 60_000;
const GROUP_ROWS: usize = 25_000;

fn write(path: &Path) {
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .set_max_row_group_row_count(Some(GROUP_ROWS))
        .build();
    let id = Int64Array::from_iter_values(0..ROWS);
    let name: StringArray = (0..ROWS).map(|i| Some(format!("row {i}"))).collect();
    let batch = RecordBatch::try_from_iter(vec![("id", Arc::new(id) as ArrayRef), ("name", Arc::new(name) as ArrayRef)])
        .unwrap();
    let mut writer = ArrowWriter::try_new(File::create(path).unwrap(), batch.schema(), Some(properties)).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
}

fn opened() -> (tempfile::TempDir, Table) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rows.parquet");
    write(&path);
    let table = Table::open(&path).unwrap();
    (dir, table)
}

fn id_at(table: &Table, row: u64) -> i64 {
    let (block, offset) = table.block_of(0, row);
    table.read(block).unwrap().as_primitive::<Int64Type>().value(offset)
}

/// Catches a row found in the wrong row group or block, which shows another
/// row's value in its place: each row's id is its own index.
#[test]
fn every_row_is_found_where_it_is() {
    let (_dir, table) = opened();
    assert_eq!(table.rows(), ROWS.cast_unsigned());
    let block = BLOCK_ROWS;
    let group = GROUP_ROWS as u64;
    for row in [0, 1, block - 1, block, block + 1, group - 1, group, group + block, 2 * group, ROWS.cast_unsigned() - 1] {
        assert_eq!(id_at(&table, row), row.cast_signed(), "row {row}");
    }
}

/// Catches a block that runs past the end of its row group, or stops short.
#[test]
fn a_block_holds_at_most_a_block_and_never_crosses_a_row_group() {
    let (_dir, table) = opened();
    let (last_of_first_group, _) = table.block_of(0, GROUP_ROWS as u64 - 1);
    let rows = table.read(last_of_first_group).unwrap().len() as u64;
    assert_eq!(rows, GROUP_ROWS as u64 % BLOCK_ROWS);
    let (first, _) = table.block_of(0, 0);
    assert_eq!(table.read(first).unwrap().len() as u64, BLOCK_ROWS);
}

/// Catches the workers never delivering, or delivering a different block than
/// was asked for.
#[test]
fn blocks_asked_for_arrive_decoded() {
    let (_dir, table) = opened();
    let table = Arc::new(table);
    let mut blocks = Blocks::new(&table, || {});
    let (wanted, offset) = table.block_of(1, 40_000);
    let started = Instant::now();
    loop {
        blocks.begin();
        if let Some(decoded) = blocks.get(wanted) {
            let array = decoded.as_ref().unwrap();
            assert_eq!(array.as_string::<i32>().value(offset), "row 40000");
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(10), "the block never arrived");
        blocks.want(vec![wanted]);
        std::thread::sleep(Duration::from_millis(5));
    }
}
