// What the file panel is told about a file, from files written here.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::builder::{Int32Builder, ListBuilder, MapBuilder, StringBuilder};
use arrow_array::{
    ArrayRef, Decimal128Array, Float64Array, Int64Array, RecordBatch, StringArray, StructArray,
    TimestampMicrosecondArray,
};
use arrow_schema::{DataType, Field as ArrowField};
use parkett::{Field, Footer};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, Repetition, ZstdLevel};
use parquet::file::metadata::KeyValue;
use parquet::file::properties::WriterProperties;

const ROWS: i64 = 100;

fn batch() -> RecordBatch {
    let ids = 0..ROWS;
    let id = Int64Array::from_iter_values(ids.clone());
    let name: StringArray = ids.clone().map(|i| (i % 10 != 0).then(|| format!("name-{i:03}"))).collect();
    let at = TimestampMicrosecondArray::from_iter_values(ids.clone().map(|i| i * 1_000_000)).with_timezone("UTC");
    let amount = Decimal128Array::from_iter_values(ids.clone().map(i128::from))
        .with_precision_and_scale(12, 2)
        .unwrap();
    let mut tags = ListBuilder::new(StringBuilder::new());
    let mut attrs = MapBuilder::new(None, StringBuilder::new(), Int32Builder::new());
    for i in ids.clone() {
        tags.values().append_value("tag");
        tags.append(true);
        attrs.keys().append_value("k");
        attrs.values().append_value(i32::try_from(i).unwrap());
        attrs.append(true).unwrap();
    }
    let x = Float64Array::from_iter_values(ids.map(|i| i as f64));
    let point = StructArray::from(vec![(
        Arc::new(ArrowField::new("x", DataType::Float64, false)),
        Arc::new(x) as ArrayRef,
    )]);
    RecordBatch::try_from_iter(vec![
        ("id", Arc::new(id) as ArrayRef),
        ("name", Arc::new(name) as ArrayRef),
        ("at", Arc::new(at) as ArrayRef),
        ("amount", Arc::new(amount) as ArrayRef),
        ("tags", Arc::new(tags.finish()) as ArrayRef),
        ("point", Arc::new(point) as ArrayRef),
        ("attrs", Arc::new(attrs.finish()) as ArrayRef),
    ])
    .unwrap()
}

fn write(path: &Path, compression: Compression) {
    let properties = WriterProperties::builder()
        .set_compression(compression)
        .set_max_row_group_row_count(Some(40))
        .set_key_value_metadata(Some(vec![KeyValue::new("origin".to_owned(), "the tests".to_owned())]))
        .build();
    let batch = batch();
    let mut writer = ArrowWriter::try_new(File::create(path).unwrap(), batch.schema(), Some(properties)).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
}

fn written(compression: Compression) -> (tempfile::TempDir, Footer) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file.parquet");
    write(&path, compression);
    let footer = Footer::read(&path).unwrap();
    (dir, footer)
}

fn field<'a>(footer: &'a Footer, name: &str) -> &'a Field {
    footer.schema.iter().find(|f| f.name == name).unwrap()
}

/// Catches rows counted per row group instead of across the file, and row
/// groups lost or merged on the way to the panel.
#[test]
fn rows_and_row_groups_are_the_files() {
    let (_dir, footer) = written(Compression::SNAPPY);
    assert_eq!(footer.rows, ROWS);
    let groups: Vec<i64> = footer.row_groups.iter().map(|g| g.rows).collect();
    assert_eq!(groups, [40, 40, 20]);
}

/// Catches a type shown as its physical storage rather than what it means:
/// a timestamp as int64, a decimal without its scale, a string as binary.
#[test]
fn a_type_is_named_by_what_it_means() {
    let (_dir, footer) = written(Compression::SNAPPY);
    assert_eq!(field(&footer, "id").kind, "int64");
    assert_eq!(field(&footer, "name").kind, "string");
    assert_eq!(field(&footer, "name").physical.as_deref(), Some("binary"));
    assert_eq!(field(&footer, "at").kind, "timestamp (µs, UTC)");
    assert_eq!(field(&footer, "amount").kind, "decimal(12, 2)");
    assert_eq!(field(&footer, "tags").kind, "list");
    assert_eq!(field(&footer, "point").kind, "struct");
    assert_eq!(field(&footer, "attrs").kind, "map");
}

/// Catches nullability read from the wrong level, or a group's children
/// dropped so a nested column's leaves are unreachable in the schema tree.
#[test]
fn the_schema_is_a_tree_with_its_nullability() {
    let (_dir, footer) = written(Compression::SNAPPY);
    assert_eq!(field(&footer, "id").repetition, Repetition::REQUIRED);
    assert_eq!(field(&footer, "name").repetition, Repetition::OPTIONAL);
    let point = field(&footer, "point");
    assert_eq!(point.children.len(), 1);
    assert_eq!(point.children[0].name, "x");
    assert_eq!(point.children[0].kind, "double");
    assert!(point.physical.is_none());
}

/// Catches the codec shown with a level the file never recorded, and a codec
/// the build lacks failing the whole footer rather than naming itself.
#[test]
fn a_codec_is_named_without_a_level() {
    let (_dir, footer) = written(Compression::ZSTD(ZstdLevel::try_new(9).unwrap()));
    for column in &footer.columns {
        assert_eq!(column.codecs, ["Zstandard"], "{}", column.path);
    }
}

/// Catches null counts summed from one row group, and min and max taken
/// from the wrong chunk or shown as bytes for a string column.
#[test]
fn statistics_are_per_row_group_and_nulls_across_the_file() {
    let (_dir, footer) = written(Compression::SNAPPY);
    let name = footer.columns.iter().position(|c| c.path == "name").unwrap();
    assert_eq!(footer.columns[name].nulls, Some(10));
    let first = &footer.row_groups[0].chunks[name];
    assert_eq!(first.min.as_deref(), Some("name-001"));
    assert_eq!(first.max.as_deref(), Some("name-039"));
    let id = footer.columns.iter().position(|c| c.path == "id").unwrap();
    let last = &footer.row_groups[2].chunks[id];
    assert_eq!((last.min.as_deref(), last.max.as_deref()), (Some("80"), Some("99")));
}

/// Catches the producer's key and value metadata dropped from the panel.
#[test]
fn key_value_metadata_is_kept() {
    let (_dir, footer) = written(Compression::SNAPPY);
    assert!(footer.metadata.contains(&("origin".to_owned(), Some("the tests".to_owned()))));
}

/// Catches a file that is not Parquet opening as an empty one.
#[test]
fn a_file_that_is_not_parquet_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not.parquet");
    std::fs::write(&path, "id,name\n1,a\n").unwrap();
    assert!(Footer::read(&path).is_err());
}
