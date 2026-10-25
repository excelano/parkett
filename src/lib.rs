//! Parkett: what a Parquet file says about itself, read from its footer.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::fmt::Write;
use std::fs::File;
use std::path::Path;

use parquet::basic::{
    Compression, ConvertedType, LogicalType, Repetition, TimeUnit, Type as PhysicalType,
};
use parquet::file::metadata::ParquetMetaData;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::file::statistics::Statistics;
use parquet::schema::types::Type;

/// How much of a statistic or a metadata value is kept for display.
const SHOWN_CHARACTERS: usize = 80;

/// Everything the footer of one file records. DESIGN.md §6.
pub struct Footer {
    pub rows: i64,
    pub writer: Option<String>,
    pub metadata: Vec<(String, Option<String>)>,
    pub schema: Vec<Field>,
    pub columns: Vec<Column>,
    pub row_groups: Vec<RowGroup>,
}

/// A field of the schema, which is a column when it has no children.
pub struct Field {
    pub name: String,
    /// The type in the words the grid's headers use. DESIGN.md §5.
    pub kind: String,
    /// The physical type a leaf is stored as; a group has none.
    pub physical: Option<String>,
    pub repetition: Repetition,
    pub children: Vec<Field>,
}

/// A leaf column across the whole file.
pub struct Column {
    pub path: String,
    pub codecs: Vec<String>,
    pub encodings: Vec<String>,
    pub compressed: i64,
    pub uncompressed: i64,
    /// `None` when any row group's statistics leave it out.
    pub nulls: Option<u64>,
}

pub struct RowGroup {
    pub rows: i64,
    pub compressed: i64,
    pub uncompressed: i64,
    /// One per leaf column, in the order of `Footer::columns`.
    pub chunks: Vec<Chunk>,
}

/// What one column chunk's statistics record.
pub struct Chunk {
    pub min: Option<String>,
    pub max: Option<String>,
    pub nulls: Option<u64>,
}

impl Footer {
    /// Read the footer of the file at `path`, and nothing else of it.
    ///
    /// # Errors
    ///
    /// When the file cannot be opened or is not Parquet; the message is the
    /// reader's.
    pub fn read(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let reader = SerializedFileReader::new(file).map_err(|e| e.to_string())?;
        Ok(Self::of(reader.metadata()))
    }

    fn of(metadata: &ParquetMetaData) -> Self {
        let file = metadata.file_metadata();
        let descriptor = file.schema_descr();
        let leaves = descriptor.columns();

        let row_groups: Vec<RowGroup> = metadata
            .row_groups()
            .iter()
            .map(|group| RowGroup {
                rows: group.num_rows(),
                compressed: group.compressed_size(),
                uncompressed: group.total_byte_size(),
                chunks: group
                    .columns()
                    .iter()
                    .map(|chunk| chunk_of(chunk.statistics(), chunk.column_descr().logical_type_ref()))
                    .collect(),
            })
            .collect();

        let columns = leaves
            .iter()
            .enumerate()
            .map(|(index, leaf)| {
                let mut column = Column {
                    path: leaf.path().string(),
                    codecs: Vec::new(),
                    encodings: Vec::new(),
                    compressed: 0,
                    uncompressed: 0,
                    nulls: Some(0),
                };
                for group in metadata.row_groups() {
                    let chunk = group.column(index);
                    push_new(&mut column.codecs, codec_name(chunk.compression()));
                    for encoding in chunk.encodings() {
                        push_new(&mut column.encodings, format!("{encoding:?}"));
                    }
                    column.compressed += chunk.compressed_size();
                    column.uncompressed += chunk.uncompressed_size();
                    column.nulls = column
                        .nulls
                        .zip(chunk.statistics().and_then(Statistics::null_count_opt))
                        .map(|(sum, nulls)| sum + nulls);
                }
                column
            })
            .collect();

        Self {
            rows: file.num_rows(),
            writer: file.created_by().map(str::to_owned),
            metadata: file
                .key_value_metadata()
                .map(|pairs| {
                    pairs
                        .iter()
                        .map(|pair| (pair.key.clone(), pair.value.as_deref().map(shortened)))
                        .collect()
                })
                .unwrap_or_default(),
            schema: descriptor.root_schema().get_fields().iter().map(|f| field_of(f)).collect(),
            columns,
            row_groups,
        }
    }
}

fn push_new(list: &mut Vec<String>, item: String) {
    if !list.contains(&item) {
        list.push(item);
    }
}

fn field_of(field: &Type) -> Field {
    let info = field.get_basic_info();
    let (kind, physical) = if field.is_group() {
        let kind = match info.logical_type_ref() {
            Some(logical) => logical_name(logical),
            None => match info.converted_type() {
                ConvertedType::LIST => "list".to_owned(),
                ConvertedType::MAP | ConvertedType::MAP_KEY_VALUE => "map".to_owned(),
                _ => "struct".to_owned(),
            },
        };
        (kind, None)
    } else {
        let physical = field.get_physical_type();
        let length = if physical == PhysicalType::FIXED_LEN_BYTE_ARRAY {
            if let Type::PrimitiveType { type_length, .. } = field {
                Some(*type_length)
            } else {
                None
            }
        } else {
            None
        };
        let kind = match info.logical_type_ref() {
            Some(logical) => logical_name(logical),
            None => converted_name(info.converted_type(), field)
                .unwrap_or_else(|| physical_name(physical, length)),
        };
        (kind, Some(physical_name(physical, length)))
    };
    Field {
        name: info.name().to_owned(),
        kind,
        physical,
        repetition: if info.has_repetition() { info.repetition() } else { Repetition::REQUIRED },
        children: if field.is_group() { field.get_fields().iter().map(|f| field_of(f)).collect() } else { Vec::new() },
    }
}

/// A logical type in the words DESIGN.md §5 uses.
fn logical_name(logical: &LogicalType) -> String {
    match logical {
        LogicalType::String => "string".to_owned(),
        LogicalType::Map => "map".to_owned(),
        LogicalType::List => "list".to_owned(),
        LogicalType::Enum => "enum".to_owned(),
        LogicalType::Decimal(decimal) => format!("decimal({}, {})", decimal.precision, decimal.scale),
        LogicalType::Date => "date".to_owned(),
        LogicalType::Time(time) => format!("time ({}{})", unit_name(time.unit), utc(time.is_adjusted_to_u_t_c)),
        LogicalType::Timestamp(time) => {
            format!("timestamp ({}{})", unit_name(time.unit), utc(time.is_adjusted_to_u_t_c))
        }
        LogicalType::Integer(int) => {
            format!("{}int{}", if int.is_signed { "" } else { "u" }, int.bit_width)
        }
        LogicalType::Unknown => "null".to_owned(),
        LogicalType::Json => "JSON".to_owned(),
        LogicalType::Bson => "BSON".to_owned(),
        LogicalType::Uuid => "UUID".to_owned(),
        LogicalType::Float16 => "float16".to_owned(),
        LogicalType::Variant(_) => "variant".to_owned(),
        LogicalType::Geometry(_) => "geometry".to_owned(),
        LogicalType::Geography(_) => "geography".to_owned(),
        LogicalType::File => "file".to_owned(),
        LogicalType::_Unknown { .. } => "unknown".to_owned(),
    }
}

fn unit_name(unit: TimeUnit) -> &'static str {
    match unit {
        TimeUnit::MILLIS => "ms",
        TimeUnit::MICROS => "µs",
        TimeUnit::NANOS => "ns",
    }
}

fn utc(adjusted: bool) -> &'static str {
    if adjusted { ", UTC" } else { "" }
}

/// The type an older writer, which records no logical type, gave a column.
fn converted_name(converted: ConvertedType, field: &Type) -> Option<String> {
    Some(match converted {
        ConvertedType::UTF8 => "string".to_owned(),
        ConvertedType::ENUM => "enum".to_owned(),
        ConvertedType::JSON => "JSON".to_owned(),
        ConvertedType::BSON => "BSON".to_owned(),
        ConvertedType::DECIMAL => format!("decimal({}, {})", field.get_precision(), field.get_scale()),
        ConvertedType::DATE => "date".to_owned(),
        ConvertedType::TIME_MILLIS => "time (ms)".to_owned(),
        ConvertedType::TIME_MICROS => "time (µs)".to_owned(),
        ConvertedType::TIMESTAMP_MILLIS => "timestamp (ms)".to_owned(),
        ConvertedType::TIMESTAMP_MICROS => "timestamp (µs)".to_owned(),
        ConvertedType::INT_8 => "int8".to_owned(),
        ConvertedType::INT_16 => "int16".to_owned(),
        ConvertedType::INT_32 => "int32".to_owned(),
        ConvertedType::INT_64 => "int64".to_owned(),
        ConvertedType::UINT_8 => "uint8".to_owned(),
        ConvertedType::UINT_16 => "uint16".to_owned(),
        ConvertedType::UINT_32 => "uint32".to_owned(),
        ConvertedType::UINT_64 => "uint64".to_owned(),
        ConvertedType::INTERVAL => "interval".to_owned(),
        _ => return None,
    })
}

fn physical_name(physical: PhysicalType, length: Option<i32>) -> String {
    match physical {
        PhysicalType::BOOLEAN => "boolean".to_owned(),
        PhysicalType::INT32 => "int32".to_owned(),
        PhysicalType::INT64 => "int64".to_owned(),
        PhysicalType::INT96 => "int96".to_owned(),
        PhysicalType::FLOAT => "float".to_owned(),
        PhysicalType::DOUBLE => "double".to_owned(),
        PhysicalType::BYTE_ARRAY => "binary".to_owned(),
        PhysicalType::FIXED_LEN_BYTE_ARRAY => match length {
            Some(length) => format!("binary({length})"),
            None => "fixed binary".to_owned(),
        },
    }
}

/// A codec's name, without the level a writer was given: the file does not
/// record it, and the reader fills in a default.
fn codec_name(compression: Compression) -> String {
    match compression {
        Compression::UNCOMPRESSED => "none",
        Compression::SNAPPY => "Snappy",
        Compression::GZIP(_) => "gzip",
        Compression::LZO => "LZO",
        Compression::BROTLI(_) => "Brotli",
        Compression::LZ4 => "LZ4 (Hadoop)",
        Compression::ZSTD(_) => "Zstandard",
        Compression::LZ4_RAW => "LZ4",
    }
    .to_owned()
}

fn chunk_of(statistics: Option<&Statistics>, logical: Option<&LogicalType>) -> Chunk {
    let Some(statistics) = statistics else {
        return Chunk { min: None, max: None, nulls: None };
    };
    // Written by writers that compared byte arrays as signed, so unreliable.
    if statistics.is_min_max_deprecated() {
        return Chunk { min: None, max: None, nulls: statistics.null_count_opt() };
    }
    let text = matches!(logical, Some(LogicalType::String | LogicalType::Enum | LogicalType::Json));
    let (min, max) = match statistics {
        Statistics::Boolean(s) => (s.min_opt().map(bool::to_string), s.max_opt().map(bool::to_string)),
        Statistics::Int32(s) => (s.min_opt().map(i32::to_string), s.max_opt().map(i32::to_string)),
        Statistics::Int64(s) => (s.min_opt().map(i64::to_string), s.max_opt().map(i64::to_string)),
        Statistics::Int96(_) => (None, None),
        Statistics::Float(s) => (s.min_opt().map(f32::to_string), s.max_opt().map(f32::to_string)),
        Statistics::Double(s) => (s.min_opt().map(f64::to_string), s.max_opt().map(f64::to_string)),
        Statistics::ByteArray(s) => (
            s.min_opt().map(|b| bytes(b.data(), text)),
            s.max_opt().map(|b| bytes(b.data(), text)),
        ),
        Statistics::FixedLenByteArray(s) => (
            s.min_opt().map(|b| bytes(b.data(), text)),
            s.max_opt().map(|b| bytes(b.data(), text)),
        ),
    };
    Chunk { min, max, nulls: statistics.null_count_opt() }
}

fn bytes(data: &[u8], text: bool) -> String {
    if text {
        shortened(&String::from_utf8_lossy(data))
    } else {
        let hex = data.iter().take(SHOWN_CHARACTERS / 2).fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        });
        if data.len() > SHOWN_CHARACTERS / 2 { format!("{hex}…") } else { hex }
    }
}

fn shortened(text: &str) -> String {
    match text.char_indices().nth(SHOWN_CHARACTERS) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}
