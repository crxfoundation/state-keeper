//! Pins each file of `crates/vendor`, the host copy of the guest engine, to its row in
//! `VENDOR.tsv`: keccak256 of the bytes, the owner path in contracts and the nearest owner commit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("vendor dir reads") {
        let p = e.expect("dir entry").path();
        if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.')) {
            continue;
        }
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
fn each_vendored_file_matches_its_manifest_row() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let manifest = std::fs::read_to_string(root.join("VENDOR.tsv")).expect("VENDOR.tsv reads");
    let mut want = BTreeMap::new();
    for line in manifest.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 7, "VENDOR.tsv row has 7 fields: {line}");
        assert_eq!(f[2], "contracts", "owner repo: {line}");
        assert!(!f[3].is_empty(), "owner path: {line}");
        for sha in [f[4], f[5]] {
            assert!(sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()), "full git id: {line}");
        }
        assert!(f[6].parse::<u32>().is_ok(), "diff-lines: {line}");
        want.insert(f[0].to_string(), f[1].to_string());
    }

    let mut all = Vec::new();
    files(&root.join("crates/vendor"), &mut all);
    let mut got = BTreeMap::new();
    for p in &all {
        let rel = p.strip_prefix(root).expect("under the root").to_str().expect("utf-8 path").replace('\\', "/");
        let bytes = std::fs::read(p).expect("vendored file reads");
        got.insert(rel, hex::encode(crx_tree::events::keccak256(&[&bytes])));
    }

    let no_file: Vec<&String> = want.keys().filter(|k| !got.contains_key(*k)).collect();
    let no_row: Vec<String> = got.iter().filter(|(k, _)| !want.contains_key(*k)).map(|(k, h)| format!("{k}\t{h}")).collect();
    let changed: Vec<String> = got
        .iter()
        .filter(|(k, h)| want.get(*k).is_some_and(|w| w != *h))
        .map(|(k, h)| format!("{k}\t{h}"))
        .collect();
    assert!(
        no_file.is_empty() && no_row.is_empty() && changed.is_empty(),
        "crates/vendor differs from VENDOR.tsv: rows with no file {no_file:?}; files with no row {no_row:?}; changed {changed:?}"
    );
    assert_eq!(got.len(), 28, "the copy holds 28 files");
}
