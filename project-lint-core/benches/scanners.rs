//! Criterion benchmarks for scanner hot paths.
//!
//! Measures `FileNamingScanner::scan` and `PatternDetector::scan_file` over a
//! synthetic project tree so regressions in the per-file scan loop are caught.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use project_lint_core::scanners::detection::{PatternDetector, PatternRule};
use project_lint_core::scanners::file_naming::FileNamingScanner;
use std::fs;
use tempfile::TempDir;

fn make_tree(file_count: usize) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    for i in 0..file_count {
        let _ = fs::write(dir.path().join(format!("file_{}.rs", i)), "fn main() {}\n");
    }
    dir
}

fn bench_file_naming_scan(c: &mut Criterion) {
    let dir = make_tree(200);
    let scanner = FileNamingScanner::new();
    c.bench_function("file_naming_scan/200-files", |b| {
        b.iter(|| black_box(scanner.scan(black_box(&dir.path().to_string_lossy()))))
    });
}

fn bench_pattern_detector_scan(c: &mut Criterion) {
    let dir = make_tree(200);
    let rules = vec![PatternRule {
        name: "todo".to_string(),
        pattern: r"TODO\(\w+\)".to_string(),
        severity: "warning".to_string(),
        message_template: "Found TODO: {matched}".to_string(),
        fix_template: None,
        case_sensitive: true,
    }];
    let detector = PatternDetector::new(rules).expect("regex");
    let file = dir.path().join("file_0.rs");
    c.bench_function("pattern_detector_scan_file", |b| {
        b.iter(|| black_box(detector.scan_file(black_box(&file))))
    });
}

criterion_group!(benches, bench_file_naming_scan, bench_pattern_detector_scan);
criterion_main!(benches);
