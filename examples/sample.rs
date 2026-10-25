//! Write a large sample file with the types the grid has to show:
//! `cargo run --release --example sample -- out.parquet [rows]`.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs::File;
use std::sync::Arc;

use arrow_array::builder::{Int32Builder, ListBuilder, MapBuilder, StringBuilder};
use arrow_array::{
    ArrayRef, Decimal128Array, Float64Array, Int64Array, RecordBatch, StringArray, StructArray,
    TimestampMicrosecondArray,
};
use arrow_schema::{DataType, Field};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

const BATCH: i64 = 100_000;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("an output path");
    let rows: i64 = args.next().map_or(5_000_000, |n| n.parse().expect("a row count"));
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap()))
        .set_max_row_group_row_count(Some(1_000_000))
        .build();
    let mut writer: Option<ArrowWriter<File>> = None;
    let mut from = 0;
    while from < rows {
        let batch = batch(from, BATCH.min(rows - from));
        let writer = writer.get_or_insert_with(|| {
            ArrowWriter::try_new(File::create(&path).unwrap(), batch.schema(), Some(properties.clone())).unwrap()
        });
        writer.write(&batch).unwrap();
        from += BATCH;
    }
    if let Some(writer) = writer {
        writer.close().unwrap();
    }
}

fn batch(from: i64, count: i64) -> RecordBatch {
    let ids = from..from + count;
    let id = Int64Array::from_iter_values(ids.clone());
    let name: StringArray = ids.clone().map(|i| (i % 7 != 0).then(|| format!("customer-{}", i % 50_000))).collect();
    let at = TimestampMicrosecondArray::from_iter_values(ids.clone().map(|i| 1_700_000_000_000_000 + i * 61_000_000))
        .with_timezone("UTC");
    let amount = Decimal128Array::from_iter_values(ids.clone().map(|i| i128::from(i * 37 % 1_000_000)))
        .with_precision_and_scale(12, 2)
        .unwrap();
    let mut tags = ListBuilder::new(StringBuilder::new());
    let mut attrs = MapBuilder::new(None, StringBuilder::new(), Int32Builder::new());
    for i in ids.clone() {
        for t in 0..i % 4 {
            tags.values().append_value(["red", "green", "blue"][usize::try_from(t % 3).unwrap()]);
        }
        tags.append(i % 11 != 0);
        attrs.keys().append_value("visits");
        attrs.values().append_value(i32::try_from(i % 100).unwrap());
        attrs.append(true).unwrap();
    }
    #[allow(clippy::cast_precision_loss)]
    let (x, y) = (
        Float64Array::from_iter_values(ids.clone().map(|i| i as f64 * 0.5)),
        Float64Array::from_iter_values(ids.map(|i| (i as f64).sqrt())),
    );
    let point = StructArray::from(vec![
        (Arc::new(Field::new("x", DataType::Float64, false)), Arc::new(x) as ArrayRef),
        (Arc::new(Field::new("y", DataType::Float64, false)), Arc::new(y) as ArrayRef),
    ]);
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
