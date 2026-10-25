// What a value looks like in a cell and in the pane beneath the grid.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::sync::Arc;

use arrow_array::builder::{Int32Builder, ListBuilder, MapBuilder, StringBuilder};
use arrow_array::{
    Array, ArrayRef, BinaryArray, Date32Array, Decimal128Array, Float64Array, StringArray, StructArray,
    TimestampMillisecondArray, TimestampNanosecondArray,
};
use arrow_schema::{DataType, Field};
use parkett::values::{is_numeric, text, whole};

/// Catches a timestamp shown in the wrong zone, unmarked as UTC, or as the
/// number it is stored as.
#[test]
fn a_timestamp_is_shown_in_its_zone() {
    let instant = 1_700_000_000_123;
    let naive = TimestampMillisecondArray::from(vec![instant]);
    assert_eq!(text(&naive, 0).unwrap(), "2023-11-14 22:13:20.123");
    let utc = TimestampMillisecondArray::from(vec![instant]).with_timezone("UTC");
    assert_eq!(text(&utc, 0).unwrap(), "2023-11-14 22:13:20.123 UTC");
    let berlin = TimestampMillisecondArray::from(vec![instant]).with_timezone("Europe/Berlin");
    assert_eq!(text(&berlin, 0).unwrap(), "2023-11-14 23:13:20.123 +01:00");
}

/// Catches a fraction that comes and goes with the value, which leaves a
/// column of timestamps ragged.
#[test]
fn a_timestamp_keeps_its_units_precision() {
    let whole_second = TimestampNanosecondArray::from(vec![1_000_000_000]);
    assert_eq!(text(&whole_second, 0).unwrap(), "1970-01-01 00:00:01.000000000");
}

/// Catches a decimal shown without its scale, a date as a day count, and
/// binary shown as text.
#[test]
fn scalars_are_shown_by_what_they_mean() {
    let decimal = Decimal128Array::from(vec![12_345]).with_precision_and_scale(10, 2).unwrap();
    assert_eq!(text(&decimal, 0).unwrap(), "123.45");
    let date = Date32Array::from(vec![19_000]);
    assert_eq!(text(&date, 0).unwrap(), "2022-01-08");
    let binary = BinaryArray::from(vec![&[0xde_u8, 0xad, 0x01][..]]);
    assert_eq!(text(&binary, 0).unwrap(), "dead01");
}

/// Catches a null shown as empty text, which cannot be told from an empty string.
#[test]
fn null_is_not_an_empty_string() {
    let strings = StringArray::from(vec![Some(""), None]);
    assert_eq!(text(&strings, 0), Some(String::new()));
    assert_eq!(text(&strings, 1), None);
}

/// Catches a nested value that is not JSON: unquoted keys, a null spelled
/// otherwise, a NaN JSON cannot hold, or an unescaped quote.
#[test]
fn a_nested_value_is_compact_json() {
    let mut list = ListBuilder::new(StringBuilder::new());
    list.values().append_value("a \"b\"");
    list.values().append_null();
    list.append(true);
    let list = list.finish();
    assert_eq!(text(&list, 0).unwrap(), r#"["a \"b\"", null]"#);

    let x = Float64Array::from(vec![f64::NAN]);
    let record = StructArray::from(vec![(Arc::new(Field::new("x", DataType::Float64, true)), Arc::new(x) as ArrayRef)]);
    assert_eq!(text(&record, 0).unwrap(), r#"{"x": "NaN"}"#);

    let mut map = MapBuilder::new(None, Int32Builder::new(), StringBuilder::new());
    map.keys().append_value(7);
    map.values().append_value("seven");
    map.append(true).unwrap();
    let map = map.finish();
    assert_eq!(text(&map, 0).unwrap(), r#"{"7": "seven"}"#);
}

/// Catches the pane showing the cell's one-line form rather than the value
/// laid out whole.
#[test]
fn a_nested_value_whole_is_indented() {
    let mut list = ListBuilder::new(Int32Builder::new());
    list.values().append_value(1);
    list.values().append_value(2);
    list.append(true);
    let list = list.finish();
    assert_eq!(whole(&list, 0).unwrap(), "[\n  1,\n  2\n]");
}

/// Catches numbers set left like text, or text set right like numbers.
#[test]
fn numbers_and_only_numbers_are_numeric() {
    assert!(is_numeric(Decimal128Array::from(vec![1]).data_type()));
    assert!(is_numeric(Float64Array::from(vec![1.0]).data_type()));
    assert!(!is_numeric(StringArray::from(vec!["1"]).data_type()));
    assert!(!is_numeric(Date32Array::from(vec![1]).data_type()));
}

/// Catches an export writing timestamps the way the grid shows them, which a
/// spreadsheet reads as text, or losing the zone on the way to ISO 8601.
#[test]
fn an_exported_timestamp_is_iso_8601() {
    use parkett::values::exported;
    let instant = 1_700_000_000_123;
    let naive = TimestampMillisecondArray::from(vec![instant]);
    assert_eq!(exported(&naive, 0).unwrap(), "2023-11-14T22:13:20.123");
    let utc = TimestampMillisecondArray::from(vec![instant]).with_timezone("UTC");
    assert_eq!(exported(&utc, 0).unwrap(), "2023-11-14T22:13:20.123Z");
    let berlin = TimestampMillisecondArray::from(vec![instant]).with_timezone("Europe/Berlin");
    assert_eq!(exported(&berlin, 0).unwrap(), "2023-11-14T23:13:20.123+01:00");
    assert_eq!(text(&utc, 0).unwrap(), "2023-11-14 22:13:20.123 UTC");
}
