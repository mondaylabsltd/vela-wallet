//! The workbook reader against real files: SheetJS-written workbooks (shared
//! strings, styles, the 1904 calendar, a second sheet) and a minimal
//! hand-built package with inline strings and no styles part.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use vela_core::xlsx::{first_sheet_rows, XlsxError};

const A: &str = "0x031d7D57c99CAF891e1C250554691Fd12D84772b";
const B: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
const C: &str = "0xee2cca98ecbff34663591a925968fa4db5a1f0dd";

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/xlsx/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn grid(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|r| r.iter().map(|c| (*c).to_owned()).collect())
        .collect()
}

#[test]
fn a_payroll_reads_as_the_values_typed_never_as_their_display_format() {
    assert_eq!(
        first_sheet_rows(&fixture("payroll-formats.xlsx")).unwrap(),
        grid(&[
            &["姓名", "address", "amount", "paid on", "ok"],
            // `0.00` would SHOW 1234.57; the person's value is 1234.567.
            &["张小明", A, "1234.567", "2024-01-31", "TRUE"],
            // A stored 0.30000000000000004 is 0.3 at Excel's 15 digits; the
            // date-time keeps its time; an #N/A cell is empty.
            &["Bob & \"Co\" <ltd>", B, "0.3", "2024-01-31 09:30:00", ""],
            // One wei, as a NUMBER cell: one significant digit, whole.
            &["Carol", C, "", "0.000000000000000001", ""],
            // Row 5 is blank and left out. A currency format adds no `$`, and
            // an 18-decimal TEXT cell arrives character for character.
            &["Dan", A, "5000", "", "0.000000000000000001"],
        ])
    );
}

#[test]
fn the_1904_calendar_and_a_sheet_that_starts_in_column_b() {
    assert_eq!(
        first_sheet_rows(&fixture("calendar-1904.xlsx")).unwrap(),
        grid(&[&["address", "amount", "date"], &[A, "42", "2024-01-31"]])
    );
}

#[test]
fn a_gap_keeps_its_column_and_every_row_is_one_width() {
    // The last row has an address and no amount: it must stay two cells wide,
    // or every later value would slide into the wrong column.
    assert_eq!(
        first_sheet_rows(&fixture("payroll-sample.xlsx")).unwrap(),
        grid(&[
            &["address", "amount"],
            &[A, "5000"],
            &[B, "173.88"],
            &[C, ""]
        ])
    );
}

#[test]
fn what_is_not_a_workbook_is_refused() {
    assert_eq!(
        first_sheet_rows(b"address,amount\n"),
        Err(XlsxError::NotZip)
    );
    let mut truncated = fixture("payroll-formats.xlsx");
    truncated.truncate(truncated.len() / 2);
    assert!(first_sheet_rows(&truncated).is_err());
}

#[test]
fn rich_text_runs_join_and_a_number_format_does_not_round() {
    // "Bob" is two runs (`Bo` + bold `b`); the amounts sit under `0.00`.
    let alice = format!("0x{}", "a1".repeat(20));
    let bob = format!("0x{}", "b2".repeat(20));
    assert_eq!(
        first_sheet_rows(&fixture("rich-text-runs.xlsx")).unwrap(),
        grid(&[
            &["name", "address", "amount"],
            &["Alice", &alice, "0.5"],
            &["Bob", &bob, "1.25"]
        ])
    );
}
