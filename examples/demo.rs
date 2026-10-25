//! Write the file the store screenshots show: hourly weather observations from
//! twenty cities, deterministic so every run writes the same bytes.
//! `cargo run --release --example demo -- packaging/demo/observations.parquet`
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs::File;
use std::sync::Arc;

use arrow_array::builder::{Float64Builder, ListBuilder, MapBuilder, StringBuilder};
use arrow_array::{
    ArrayRef, Decimal128Array, Float64Array, Int32Array, RecordBatch, StringArray, StructArray,
    TimestampMicrosecondArray,
};
use arrow_schema::{DataType, Field};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

const HOURS: usize = 2_500;
/// 2026-01-01T00:00:00Z.
const START: i64 = 1_767_225_600_000_000;
const HOUR: i64 = 3_600_000_000;

const CITIES: &[(&str, &str, f64, f64, f64)] = &[
    ("Berlin", "DE", 52.52, 13.405, 1.5),
    ("Hamburg", "DE", 53.551, 9.993, 2.0),
    ("München", "DE", 48.137, 11.575, 0.0),
    ("Köln", "DE", 50.937, 6.96, 3.0),
    ("Wien", "AT", 48.208, 16.373, 1.0),
    ("Zürich", "CH", 47.376, 8.541, 0.5),
    ("Amsterdam", "NL", 52.368, 4.904, 4.0),
    ("Brussels", "BE", 50.85, 4.352, 4.0),
    ("Paris", "FR", 48.857, 2.352, 5.0),
    ("Lyon", "FR", 45.764, 4.836, 4.5),
    ("Madrid", "ES", 40.417, -3.704, 7.0),
    ("Lisbon", "PT", 38.722, -9.139, 11.0),
    ("Rome", "IT", 41.903, 12.496, 8.5),
    ("Milan", "IT", 45.464, 9.19, 3.5),
    ("Prague", "CZ", 50.075, 14.438, 0.5),
    ("Warsaw", "PL", 52.23, 21.012, -1.0),
    ("Copenhagen", "DK", 55.676, 12.568, 2.0),
    ("Stockholm", "SE", 59.329, 18.069, -1.5),
    ("Oslo", "NO", 59.914, 10.752, -3.0),
    ("Dublin", "IE", 53.35, -6.26, 5.5),
];

/// A small deterministic generator, so the file is the same on every machine.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        #[allow(clippy::cast_precision_loss)]
        let unit = (self.0 >> 11) as f64 / (1_u64 << 53) as f64;
        unit
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("an output path");
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap()))
        .set_max_row_group_row_count(Some(10_000))
        .set_created_by("Parkett demo".to_owned())
        .build();
    let batch = batch();
    let mut writer = ArrowWriter::try_new(File::create(&path).unwrap(), batch.schema(), Some(properties)).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn batch() -> RecordBatch {
    let mut noise = Noise(2026);
    let (mut station, mut city, mut country, mut at) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut temperature, mut precipitation, mut lat, mut lon) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut conditions = ListBuilder::new(StringBuilder::new());
    let mut sensors = MapBuilder::new(None, StringBuilder::new(), Float64Builder::new());
    for hour in 0..HOURS {
        for (index, &(name, code, latitude, longitude, mean)) in CITIES.iter().enumerate() {
            let day = (hour as f64 / 24.0) * std::f64::consts::TAU / 365.0;
            let daily = ((hour % 24) as f64 - 9.0) / 24.0 * std::f64::consts::TAU;
            let t = mean + 9.0 * (1.0 + (day - 1.9).sin()) + 3.0 * daily.sin() + 3.0 * (noise.next() - 0.5);
            let rain = if noise.next() < 0.18 { (noise.next() * 60.0).round() / 10.0 } else { 0.0 };
            station.push(1001 + index as i32);
            city.push((noise.next() > 0.002).then_some(name));
            country.push(code);
            at.push(START + hour as i64 * HOUR);
            temperature.push((t * 10.0).round() / 10.0);
            precipitation.push((rain * 10.0).round() as i128);
            lat.push(latitude);
            lon.push(longitude);
            if rain > 0.0 {
                conditions.values().append_value(if t < 0.5 { "snow" } else { "rain" });
            }
            if noise.next() < 0.45 {
                conditions.values().append_value("cloudy");
            } else if rain == 0.0 {
                conditions.values().append_value("clear");
            }
            if noise.next() < 0.08 {
                conditions.values().append_value("wind");
            }
            conditions.append(true);
            sensors.keys().append_value("wind_kmh");
            sensors.values().append_value((noise.next() * 400.0).round() / 10.0);
            sensors.keys().append_value("pressure_hpa");
            sensors.values().append_value(((990.0 + noise.next() * 40.0) * 10.0).round() / 10.0);
            if noise.next() < 0.97 {
                sensors.keys().append_value("humidity_pct");
                sensors.values().append_value((40.0 + noise.next() * 55.0).round());
            }
            sensors.append(true).unwrap();
        }
    }
    let location = StructArray::from(vec![
        (Arc::new(Field::new("lat", DataType::Float64, false)), Arc::new(Float64Array::from(lat)) as ArrayRef),
        (Arc::new(Field::new("lon", DataType::Float64, false)), Arc::new(Float64Array::from(lon)) as ArrayRef),
    ]);
    RecordBatch::try_from_iter(vec![
        ("station", Arc::new(Int32Array::from(station)) as ArrayRef),
        ("city", Arc::new(StringArray::from(city)) as ArrayRef),
        ("country", Arc::new(StringArray::from(country)) as ArrayRef),
        ("observed_at", Arc::new(TimestampMicrosecondArray::from(at).with_timezone("UTC")) as ArrayRef),
        ("temperature_c", Arc::new(Float64Array::from(temperature)) as ArrayRef),
        (
            "precipitation_mm",
            Arc::new(Decimal128Array::from(precipitation).with_precision_and_scale(6, 1).unwrap()) as ArrayRef,
        ),
        ("conditions", Arc::new(conditions.finish()) as ArrayRef),
        ("location", Arc::new(location) as ArrayRef),
        ("sensors", Arc::new(sensors.finish()) as ArrayRef),
    ])
    .unwrap()
}
