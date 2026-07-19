//! Integration tests for the IEM-Tool-rs utility crate (manifest + converter).
use iem_utils::*;
use std::fs;

#[test]
fn parse_handles_mixed_separators() {
    let body = "# header line\n20 -9.5\n100,\t2.0\n1000;3\n\nbad line here\n20000  -4\n";
    let pts = parse_measurement(body);
    assert_eq!(pts.len(), 4);
    assert_eq!(pts[0], (20.0, -9.5));
    assert_eq!(pts[1], (100.0, 2.0));
    assert_eq!(pts[2], (1000.0, 3.0));
    assert_eq!(pts[3], (20000.0, -4.0));
}

#[test]
fn channel_detection() {
    assert_eq!(
        channel_of("SOME IEM [1]"),
        ("SOME IEM".to_string(), Some('1'))
    );
    assert_eq!(
        channel_of("SOME IEM [2]"),
        ("SOME IEM".to_string(), Some('2'))
    );
    assert_eq!(channel_of("SOME IEM"), ("SOME IEM".to_string(), None));
}

#[test]
fn standardize_name_strips_channel_and_uppercases() {
    assert_eq!(standardize_name("moondrop aria [1]"), "MOONDROP ARIA");
    assert_eq!(standardize_name("  Sony   WF 1000XM4 "), "SONY WF 1000XM4");
}

#[test]
fn average_pair_midpoint() {
    let a = vec![(20.0, -10.0), (1000.0, 4.0), (10000.0, -2.0)];
    let b = vec![(20.0, -8.0), (1000.0, 6.0), (10000.0, -6.0)];
    let m = average_pair(&a, &b);
    assert_eq!(m[0], (20.0, -9.0));
    assert_eq!(m[1], (1000.0, 5.0));
    assert_eq!(m[2], (10000.0, -4.0));
}

#[test]
fn manifest_scan_sorts_and_sizes() {
    let dir = std::env::temp_dir().join(format!("iemtest_{}", std::process::id()));
    let data = dir.join("data");
    fs::create_dir_all(data.join("BRAND B")).unwrap();
    fs::create_dir_all(data.join("BRAND A")).unwrap();
    fs::write(data.join("BRAND B/ZED.txt"), "20 0\n").unwrap(); // 5 bytes
    fs::write(data.join("BRAND A/ALPHA.txt"), "20 0\n1000 1\n").unwrap();
    fs::write(data.join("BRAND A/note.md"), "ignore me").unwrap(); // non-txt

    let entries = generate_manifest(&data, "data").unwrap();
    assert_eq!(entries.len(), 2, "only .txt files counted");
    // sorted by path: BRAND A before BRAND B
    assert_eq!(entries[0].file, "data/BRAND A/ALPHA.txt");
    assert_eq!(entries[1].file, "data/BRAND B/ZED.txt");
    assert_eq!(
        entries[1].size,
        fs::metadata(data.join("BRAND B/ZED.txt")).unwrap().len()
    );

    // JSON shape
    let json = manifest_to_json(&entries);
    assert!(json.starts_with("[\n  {\n    \"file\": \"data/BRAND A/ALPHA.txt\","));
    assert!(json.trim_end().ends_with("]"));
    // parses back with the same count
    let count = json.matches("\"file\"").count();
    assert_eq!(count, 2);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_manifest_is_empty_array() {
    assert_eq!(manifest_to_json(&[]), "[]");
}
