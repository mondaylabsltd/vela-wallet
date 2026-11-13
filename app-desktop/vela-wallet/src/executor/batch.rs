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
//! ## A workbook is the core's to read
//!
//! The picked file goes over as its bytes, whatever kind it is: the core
//! tells a workbook (`vela_core::xlsx`, the reader every platform shares) from
//! a text table, and reads either. `calamine` read workbooks here once, and
//! each platform's reader rendered the same payroll its own way.

use std::path::Path;

use vela_core::app::batch_import::BatchFileContent;

use crate::executor::display_currency;

/// Read the table a person picked: its bytes, undecoded — the core reads a
/// workbook itself and decodes text once (UTF-8, or UTF-16 by its BOM, never a
/// guess), as it does a contacts file (087, issue 333's twin). `None` =
/// unreadable, which the host answers as `FilePickFailed`. `read_to_string`
/// used to decode here: strict, so a GBK CSV was refused as "use a CSV" (which
/// it is), and a UTF-16 one could not be read at all.
pub fn read_table(path: &Path) -> Option<BatchFileContent> {
    let bytes = std::fs::read(path).ok()?;
    Some(BatchFileContent::Bytes { bytes })
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

    /// Text tables go to the core as their bytes — the encoding, the
    /// delimiter, the header and the amount column are its to decide (087).
    #[test]
    fn a_csv_is_handed_over_as_its_bytes() {
        let path = temp("payroll.csv", b"address,amount\n0xabc,5000\n");
        match read_table(&path) {
            Some(BatchFileContent::Bytes { bytes }) => {
                assert_eq!(bytes, b"address,amount\n0xabc,5000\n");
            }
            other => unreachable!("{other:?}"),
        }
        // A GBK name is not decoded (or refused) here: its bytes go as they are.
        let gbk = temp("gbk.csv", &[0xD5, 0xC5, 0xC8, 0xFD]);
        assert!(matches!(
            read_table(&gbk),
            Some(BatchFileContent::Bytes { bytes }) if bytes == [0xD5, 0xC5, 0xC8, 0xFD]
        ));
        assert_eq!(file_name(&path), "payroll.csv");
        assert!(read_table(Path::new("/nonexistent/vela.csv")).is_none());
    }

    /// A REAL workbook goes over as its bytes, and the core reads it into
    /// the grid it parses — the money path "bring your payroll from Excel",
    /// end to end through this file.
    #[test]
    fn a_real_workbook_goes_over_as_its_bytes_and_the_core_reads_it() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/payroll-sample.xlsx");
        let Some(BatchFileContent::Bytes { bytes }) = read_table(&path) else {
            unreachable!("a workbook reads as its bytes")
        };
        let rows = vela_core::xlsx::first_sheet_rows(&bytes)
            .unwrap_or_else(|error| unreachable!("the fixture is a workbook: {error}"));
        assert_eq!(
            rows,
            vec![
                vec!["address".to_owned(), "amount".to_owned()],
                vec![
                    "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned(),
                    "5000".to_owned(),
                ],
                vec![
                    "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
                    "173.88".to_owned(),
                ],
                // A row with one cell in a two-column sheet keeps both columns,
                // so the amount column stays the amount column.
                vec![
                    "0xee2cca98ecbff34663591a925968fa4db5a1f0dd".to_owned(),
                    String::new(),
                ],
            ]
        );
    }

    #[test]
    fn usd_prices_itself() {
        assert_eq!(usd_fiat_rate("USD"), Some(1.0));
    }
}
