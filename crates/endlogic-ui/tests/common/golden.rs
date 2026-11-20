//! Golden files: expected results kept in the repository next to the tests.
//!
//! A test computes its result as text and hands it to [`check`]. Normally
//! the text must equal the file in `tests/golden`; when it does not, the
//! actual result is written under the target directory and the test fails
//! with the first differences and both paths. With `ENDLOGIC_BLESS=1` in the
//! environment (`just bless`) the file is rewritten instead.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The environment variable that rewrites golden files instead of checking.
pub const BLESS: &str = "ENDLOGIC_BLESS";

/// Whether this run rewrites golden files.
pub fn blessing() -> bool {
    std::env::var_os(BLESS).is_some_and(|v| v == "1")
}

/// Where the golden file `name` lives in the repository.
pub fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(name)
}

/// Where the actual result for `name` is written when it differs.
pub fn actual_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("golden")
        .join(env!("CARGO_PKG_NAME"))
        .join(name)
}

pub fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Compares `actual` with the golden file `name`, or rewrites the file when
/// blessing.
pub fn check(name: &str, actual: &str) {
    let expected_path = golden_path(name);
    if blessing() {
        write(&expected_path, actual);
        return;
    }
    let Ok(expected) = std::fs::read_to_string(&expected_path) else {
        let out = actual_path(name);
        write(&out, actual);
        panic!(
            "golden file {} is missing; the actual result is in {}; \
             run `just bless` to create it",
            expected_path.display(),
            out.display()
        );
    };
    let expected = expected.replace("\r\n", "\n");
    if expected == actual {
        return;
    }
    let out = actual_path(name);
    write(&out, actual);
    panic!(
        "{}\nexpected: {}\nactual:   {}\nif the change is intended, run `just bless`",
        differences(&expected, actual),
        expected_path.display(),
        out.display()
    );
}

/// The first few lines that differ, numbered from 1.
pub fn differences(expected: &str, actual: &str) -> String {
    let (old, new): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), actual.lines().collect());
    let mut text = format!(
        "the result differs from the golden file ({} lines expected, {} actual)",
        old.len(),
        new.len()
    );
    let mut shown = 0;
    let mut differing = 0;
    for i in 0..old.len().max(new.len()) {
        let (a, b) = (old.get(i).copied(), new.get(i).copied());
        if a == b {
            continue;
        }
        differing += 1;
        if shown < 6 {
            shown += 1;
            let _ = write!(
                text,
                "\n  line {}:\n    expected: {}\n    actual:   {}",
                i + 1,
                a.unwrap_or("(none)"),
                b.unwrap_or("(none)")
            );
        }
    }
    let _ = write!(text, "\n  {differing} lines differ in all");
    text
}
