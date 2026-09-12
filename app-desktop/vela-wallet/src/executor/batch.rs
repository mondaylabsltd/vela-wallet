//! The only place the `batch_import` machine touches the outside world.
//!
//! Three operations, three capabilities the core cannot have: the USD→fiat
//! rate source, a file, and a saved file. Nothing here branches on business
//! meaning — which rows a table has, what a column means, whether a rate for
//! a currency the person has since switched away from counts, and whether a
//! batch may be applied at all are `batch_import.rs`'s 1,648 lines.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/flows/core/batch-executor.ts`
//! and `services/file-io.ts` @ `origin/main`. The two dialogs (open, save)
//! belong to the host, which holds the gpui context they need; what is here
//! is the reading and the writing on either side of them.
//!
//! ## The rate is `resolve_rate`, never a display helper
//!
//! `None` is the honest observation — "no source can price it" — and the
//! core turns it into `Failed` → an empty rate → `can_apply: false`. A helper
//! that ends in `?? 1` would arrive as "the rate really is 1", and a 5,000 CNY
//! payroll line would be split at 1 CNY = 1 token, ~7× the intended payout,
//! behind a green button. Same choice as the display-currency executor.
//!
//! ## A workbook is a cell matrix, nothing more
//!
//! `calamine` reads the first sheet; every cell becomes the text the core
//! would have seen in a CSV (`String(cell ?? '')`), ragged rows are padded,
//! and the trim stays in the core. The 324-line TypeScript column interpreter
//! that once sat here has no reader on any tier.

use std::path::Path;

use vela_core::app::batch_import::BatchFileContent;

use crate::executor::display_currency;

// gpui's open dialog takes no extension filter (`TABLE_ACCEPT` on the web);
// what was picked is classified by name below and read accordingly.
fn is_excel(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["xlsx", "xlsm", "xlsb", "xls"]
                .iter()
                .any(|excel| ext.eq_ignore_ascii_case(excel))
        })
}

/// A workbook's first sheet as the core's matrix. Every cell is text; a
/// short row is padded to the widest so column positions stay stable
/// (SheetJS's `defval: ''`).
pub fn workbook_matrix(path: &Path) -> Option<Vec<Vec<String>>> {
    use calamine::Reader as _;
    let mut workbook = calamine::open_workbook_auto(path).ok()?;
    let range = workbook.worksheet_range_at(0)?.ok()?;
    let width = range.width();
    let rows: Vec<Vec<String>> = range
        .rows()
        .map(|row| {
            let mut cells: Vec<String> = row.iter().map(cell_text).collect();
            cells.resize(width, String::new());
            cells
        })
        .collect();
    Some(rows)
}

/// One cell as the text a CSV would carry. An integral float prints without
/// its `.0` — `5000` typed into a spreadsheet is a number to the sheet and
/// `5000` to the person, and the core's amount parser wants the latter.
fn cell_text(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Empty => String::new(),
        calamine::Data::Float(value) if value.fract() == 0.0 && value.abs() < 1e15 => {
            format!("{}", *value as i64)
        }
        other => other.to_string(),
    }
}

/// Read the table a person picked: text for CSV/TSV/TXT (the core parses
/// it), a matrix for a workbook. `None` = unreadable, which the host answers
/// as `FilePickFailed`.
pub fn read_table(path: &Path) -> Option<BatchFileContent> {
    if is_excel(path) {
        return workbook_matrix(path).map(|rows| BatchFileContent::Matrix { rows });
    }
    let text = std::fs::read_to_string(path).ok()?;
    Some(BatchFileContent::Text { text })
}

/// The file's own name, for the sheet's "picked" line.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// USD → `code`, or `None` when no source can price it.
pub fn usd_fiat_rate(code: &str) -> Option<f64> {
    display_currency::resolve_rate(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("vela-batch-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap_or_else(|e| unreachable!("{e}"));
        path
    }

    /// Text tables go to the core as text — the delimiter, the header and
    /// the amount column are its to decide.
    #[test]
    fn a_csv_is_handed_over_as_text() {
        let path = temp("payroll.csv", b"address,amount\n0xabc,5000\n");
        match read_table(&path) {
            Some(BatchFileContent::Text { text }) => assert!(text.starts_with("address,amount")),
            other => unreachable!("{other:?}"),
        }
        assert_eq!(file_name(&path), "payroll.csv");
        assert!(read_table(Path::new("/nonexistent/vela.csv")).is_none());
    }

    #[test]
    fn excel_is_recognised_by_extension_only() {
        assert!(is_excel(Path::new("a.XLSX")));
        assert!(is_excel(Path::new("a.xlsm")));
        assert!(!is_excel(Path::new("a.csv")));
        assert!(!is_excel(Path::new("xlsx")));
        // A workbook that is not one reads as unreadable, never as an empty
        // matrix the core would parse into "no rows".
        let fake = temp("not-really.xlsx", b"this is not a zip");
        assert!(read_table(&fake).is_none());
    }

    /// A number in a sheet is `5000` to the person and must stay so; a
    /// fraction keeps its digits; the empty cell is the empty string.
    /// A REAL workbook, opened by calamine, end to end.
    ///
    /// Everything above tests the pieces: the extension test, the cell
    /// formatter, a file that is not a zip. None of them ever opened a
    /// workbook, so the one line that matters — `open_workbook_auto` into
    /// `worksheet_range_at(0)` — had never run. The fixture is committed
    /// beside this file because "bring your payroll from Excel" is a money
    /// path and it deserves a table it has actually read.
    #[test]
    fn a_real_workbook_becomes_the_matrix_the_core_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/payroll-sample.xlsx");
        let Some(BatchFileContent::Matrix { rows }) = read_table(&path) else {
            unreachable!("a workbook reads as a matrix")
        };
        assert_eq!(
            rows,
            vec![
                vec!["address".to_owned(), "amount".to_owned()],
                vec![
                    "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned(),
                    // 5000 is a NUMBER to the sheet; the core's amount parser
                    // wants what the person typed, not "5000.0".
                    "5000".to_owned(),
                ],
                vec![
                    "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
                    // …and trimming the `.0` must not truncate a real decimal.
                    "173.88".to_owned(),
                ],
                // A row with one cell in a two-column sheet is PADDED, so the
                // amount column stays the amount column. An unpadded short row
                // would slide the next value into somebody's amount.
                vec![
                    "0xee2cca98ecbff34663591a925968fa4db5a1f0dd".to_owned(),
                    String::new(),
                ],
            ]
        );
    }

    #[test]
    fn cells_read_as_the_text_a_csv_would_carry() {
        assert_eq!(cell_text(&calamine::Data::Float(5000.0)), "5000");
        assert_eq!(cell_text(&calamine::Data::Float(173.88)), "173.88");
        assert_eq!(cell_text(&calamine::Data::Int(7)), "7");
        assert_eq!(
            cell_text(&calamine::Data::String("0xabc".to_owned())),
            "0xabc"
        );
        assert_eq!(cell_text(&calamine::Data::Empty), "");
    }

    #[test]
    fn usd_prices_itself() {
        assert_eq!(usd_fiat_rate("USD"), Some(1.0));
    }
}
