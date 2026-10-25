//! A value as text, in the form its type gives it. DESIGN.md §5.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fmt::Write;

use arrow_array::cast::AsArray;
use arrow_array::temporal_conversions::{as_date, as_datetime, as_datetime_with_timezone, as_duration, as_time};
use arrow_array::timezone::Tz;
use arrow_array::types::{
    Date32Type, Date64Type, Decimal32Type, Decimal64Type, Decimal128Type, Decimal256Type, DurationMicrosecondType,
    DurationMillisecondType, DurationNanosecondType, DurationSecondType, Float16Type, Float32Type, Float64Type,
    Int8Type, Int16Type, Int32Type, Int64Type, Time32MillisecondType, Time32SecondType, Time64MicrosecondType,
    Time64NanosecondType, TimestampMicrosecondType, TimestampMillisecondType, TimestampNanosecondType,
    TimestampSecondType, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use arrow_array::{Array, ArrowPrimitiveType};
use arrow_schema::{DataType, TimeUnit};

/// The value at `row` on one line, or `None` for null.
#[must_use]
pub fn text(array: &dyn Array, row: usize) -> Option<String> {
    value(array, row, None)
}

/// The value at `row` whole, with a nested value indented, or `None` for null.
#[must_use]
pub fn whole(array: &dyn Array, row: usize) -> Option<String> {
    value(array, row, Some(2))
}

/// Whether a column's values are numbers, which the grid sets right-aligned.
#[must_use]
pub fn is_numeric(data_type: &DataType) -> bool {
    match data_type {
        DataType::Dictionary(_, values) => is_numeric(values),
        other => other.is_numeric(),
    }
}

fn value(array: &dyn Array, row: usize, indent: Option<usize>) -> Option<String> {
    if array.is_null(row) {
        return None;
    }
    if let Some((values, at)) = dictionary_value(array, row) {
        return value(values, at, indent);
    }
    if is_nested(array.data_type()) {
        let mut out = String::new();
        json(array, row, &mut out, indent, 0);
        Some(out)
    } else {
        scalar(array, row)
    }
}

fn is_nested(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::List(_)
            | DataType::LargeList(_)
            | DataType::ListView(_)
            | DataType::LargeListView(_)
            | DataType::FixedSizeList(..)
            | DataType::Struct(_)
            | DataType::Map(..)
    )
}

/// The values array of a dictionary and the index into it of `row`'s value.
fn dictionary_value(array: &dyn Array, row: usize) -> Option<(&dyn Array, usize)> {
    let DataType::Dictionary(key, _) = array.data_type() else { return None };
    macro_rules! keyed {
        ($($key:ident => $type:ty),*) => {
            match key.as_ref() {
                $(DataType::$key => {
                    let dictionary = array.as_dictionary::<$type>();
                    dictionary.key(row).map(|at| (dictionary.values().as_ref(), at))
                })*
                _ => None,
            }
        };
    }
    keyed!(Int8 => Int8Type, Int16 => Int16Type, Int32 => Int32Type, Int64 => Int64Type,
        UInt8 => UInt8Type, UInt16 => UInt16Type, UInt32 => UInt32Type, UInt64 => UInt64Type)
}

fn primitive<T: ArrowPrimitiveType>(array: &dyn Array, row: usize) -> T::Native {
    array.as_primitive::<T>().value(row)
}

fn scalar(array: &dyn Array, row: usize) -> Option<String> {
    Some(match array.data_type() {
        DataType::Null => return None,
        DataType::Boolean => array.as_boolean().value(row).to_string(),
        DataType::Int8 => primitive::<Int8Type>(array, row).to_string(),
        DataType::Int16 => primitive::<Int16Type>(array, row).to_string(),
        DataType::Int32 => primitive::<Int32Type>(array, row).to_string(),
        DataType::Int64 => primitive::<Int64Type>(array, row).to_string(),
        DataType::UInt8 => primitive::<UInt8Type>(array, row).to_string(),
        DataType::UInt16 => primitive::<UInt16Type>(array, row).to_string(),
        DataType::UInt32 => primitive::<UInt32Type>(array, row).to_string(),
        DataType::UInt64 => primitive::<UInt64Type>(array, row).to_string(),
        DataType::Float16 => primitive::<Float16Type>(array, row).to_string(),
        DataType::Float32 => primitive::<Float32Type>(array, row).to_string(),
        DataType::Float64 => primitive::<Float64Type>(array, row).to_string(),
        DataType::Decimal32(..) => array.as_primitive::<Decimal32Type>().value_as_string(row),
        DataType::Decimal64(..) => array.as_primitive::<Decimal64Type>().value_as_string(row),
        DataType::Decimal128(..) => array.as_primitive::<Decimal128Type>().value_as_string(row),
        DataType::Decimal256(..) => array.as_primitive::<Decimal256Type>().value_as_string(row),
        DataType::Utf8 => array.as_string::<i32>().value(row).to_owned(),
        DataType::LargeUtf8 => array.as_string::<i64>().value(row).to_owned(),
        DataType::Utf8View => array.as_string_view().value(row).to_owned(),
        DataType::Binary => hex(array.as_binary::<i32>().value(row)),
        DataType::LargeBinary => hex(array.as_binary::<i64>().value(row)),
        DataType::BinaryView => hex(array.as_binary_view().value(row)),
        DataType::FixedSizeBinary(_) => hex(array.as_fixed_size_binary().value(row)),
        DataType::Date32 => shown(as_date::<Date32Type>(i64::from(primitive::<Date32Type>(array, row)))),
        DataType::Date64 => shown(as_date::<Date64Type>(primitive::<Date64Type>(array, row))),
        DataType::Time32(TimeUnit::Second) => {
            shown(as_time::<Time32SecondType>(i64::from(primitive::<Time32SecondType>(array, row))))
        }
        DataType::Time32(_) => {
            shown(as_time::<Time32MillisecondType>(i64::from(primitive::<Time32MillisecondType>(array, row))))
        }
        DataType::Time64(TimeUnit::Nanosecond) => {
            shown(as_time::<Time64NanosecondType>(primitive::<Time64NanosecondType>(array, row)))
        }
        DataType::Time64(_) => shown(as_time::<Time64MicrosecondType>(primitive::<Time64MicrosecondType>(array, row))),
        DataType::Timestamp(unit, zone) => {
            let raw = match unit {
                TimeUnit::Second => primitive::<TimestampSecondType>(array, row),
                TimeUnit::Millisecond => primitive::<TimestampMillisecondType>(array, row),
                TimeUnit::Microsecond => primitive::<TimestampMicrosecondType>(array, row),
                TimeUnit::Nanosecond => primitive::<TimestampNanosecondType>(array, row),
            };
            timestamp(raw, *unit, zone.as_deref())
        }
        DataType::Duration(unit) => {
            let duration = match unit {
                TimeUnit::Second => as_duration::<DurationSecondType>(primitive::<DurationSecondType>(array, row)),
                TimeUnit::Millisecond => {
                    as_duration::<DurationMillisecondType>(primitive::<DurationMillisecondType>(array, row))
                }
                TimeUnit::Microsecond => {
                    as_duration::<DurationMicrosecondType>(primitive::<DurationMicrosecondType>(array, row))
                }
                TimeUnit::Nanosecond => {
                    as_duration::<DurationNanosecondType>(primitive::<DurationNanosecondType>(array, row))
                }
            };
            shown(duration)
        }
        other => format!("({other} is not shown)"),
    })
}

fn shown<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "(out of range)".to_owned(), |v| v.to_string())
}

/// A timestamp to the precision its unit records.
fn date_time(unit: TimeUnit) -> &'static str {
    match unit {
        TimeUnit::Second => "%Y-%m-%d %H:%M:%S",
        TimeUnit::Millisecond => "%Y-%m-%d %H:%M:%S%.3f",
        TimeUnit::Microsecond => "%Y-%m-%d %H:%M:%S%.6f",
        TimeUnit::Nanosecond => "%Y-%m-%d %H:%M:%S%.9f",
    }
}

fn timestamp(raw: i64, unit: TimeUnit, zone: Option<&str>) -> String {
    let pattern = date_time(unit);
    macro_rules! at {
        ($type:ty) => {
            match zone {
                None => as_datetime::<$type>(raw).map(|t| t.format(pattern).to_string()),
                Some("UTC" | "Etc/UTC" | "Z" | "+00:00" | "+00") => {
                    as_datetime::<$type>(raw).map(|t| format!("{} UTC", t.format(pattern)))
                }
                Some(zone) => match zone.parse::<Tz>() {
                    Ok(tz) => as_datetime_with_timezone::<$type>(raw, tz)
                        .map(|t| t.format(&format!("{pattern} %:z")).to_string()),
                    Err(_) => as_datetime::<$type>(raw).map(|t| format!("{} ({zone}?)", t.format(pattern))),
                },
            }
        };
    }
    let text = match unit {
        TimeUnit::Second => at!(TimestampSecondType),
        TimeUnit::Millisecond => at!(TimestampMillisecondType),
        TimeUnit::Microsecond => at!(TimestampMicrosecondType),
        TimeUnit::Nanosecond => at!(TimestampNanosecondType),
    };
    text.unwrap_or_else(|| "(out of range)".to_owned())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

/// A value as JSON: compact when `indent` is `None`, else indented by it.
fn json(array: &dyn Array, row: usize, out: &mut String, indent: Option<usize>, depth: usize) {
    if array.is_null(row) {
        out.push_str("null");
        return;
    }
    if let Some((values, at)) = dictionary_value(array, row) {
        return json(values, at, out, indent, depth);
    }
    match array.data_type() {
        DataType::List(_) => {
            let items = array.as_list::<i32>().value(row);
            sequence(out, indent, depth, '[', ']', (0..items.len()).map(|i| (None, items.as_ref(), i)));
        }
        DataType::LargeList(_) => {
            let items = array.as_list::<i64>().value(row);
            sequence(out, indent, depth, '[', ']', (0..items.len()).map(|i| (None, items.as_ref(), i)));
        }
        DataType::ListView(_) => {
            let items = array.as_list_view::<i32>().value(row);
            sequence(out, indent, depth, '[', ']', (0..items.len()).map(|i| (None, items.as_ref(), i)));
        }
        DataType::LargeListView(_) => {
            let items = array.as_list_view::<i64>().value(row);
            sequence(out, indent, depth, '[', ']', (0..items.len()).map(|i| (None, items.as_ref(), i)));
        }
        DataType::FixedSizeList(..) => {
            let items = array.as_fixed_size_list().value(row);
            sequence(out, indent, depth, '[', ']', (0..items.len()).map(|i| (None, items.as_ref(), i)));
        }
        DataType::Struct(fields) => {
            let record = array.as_struct();
            sequence(
                out,
                indent,
                depth,
                '{',
                '}',
                fields.iter().zip(record.columns()).map(|(field, column)| {
                    (Some(field.name().clone()), column.as_ref(), row)
                }),
            );
        }
        DataType::Map(..) => {
            let entries = array.as_map().value(row);
            let (keys, values) = (entries.column(0), entries.column(1));
            sequence(
                out,
                indent,
                depth,
                '{',
                '}',
                (0..entries.len()).map(|i| (Some(text(keys.as_ref(), i).unwrap_or_default()), values.as_ref(), i)),
            );
        }
        DataType::Boolean
        | DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Decimal32(..)
        | DataType::Decimal64(..)
        | DataType::Decimal128(..)
        | DataType::Decimal256(..) => out.push_str(&scalar(array, row).unwrap_or_default()),
        DataType::Float16 | DataType::Float32 | DataType::Float64 => {
            let number = scalar(array, row).unwrap_or_default();
            if number.parse::<f64>().is_ok_and(f64::is_finite) {
                out.push_str(&number);
            } else {
                quoted(out, &number);
            }
        }
        _ => quoted(out, &scalar(array, row).unwrap_or_default()),
    }
}

fn sequence<'a>(
    out: &mut String,
    indent: Option<usize>,
    depth: usize,
    open: char,
    close: char,
    items: impl Iterator<Item = (Option<String>, &'a dyn Array, usize)>,
) {
    out.push(open);
    let mut empty = true;
    for (key, array, row) in items {
        if !empty {
            out.push(',');
            if indent.is_none() {
                out.push(' ');
            }
        }
        empty = false;
        if let Some(width) = indent {
            out.push('\n');
            out.extend(std::iter::repeat_n(' ', width * (depth + 1)));
        }
        if let Some(key) = key {
            quoted(out, &key);
            out.push_str(": ");
        }
        json(array, row, out, indent, depth + 1);
    }
    if let (Some(width), false) = (indent, empty) {
        out.push('\n');
        out.extend(std::iter::repeat_n(' ', width * depth));
    }
    out.push(close);
}

fn quoted(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
