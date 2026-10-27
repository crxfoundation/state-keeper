//! `crates/vendor` is the host copy of the guest engine whose ELF `elf/scenario-es-program`
//! pins (sha256 6068e927…, vkey 0x005283d6…). The copy carries no own tests in the default run;
//! this pin fails on any byte change to it.

const VENDOR_KECCAK: &str = "c5b18810666a07eb9598049dc7b17acf3d5b036c134c9f9d592082553b949c32";

fn files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).expect("vendor dir reads") {
        let p = e.expect("dir entry").path();
        let hidden = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.'));
        if hidden {
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
fn the_vendored_engine_is_byte_equal_to_its_pin() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../vendor"));
    let mut all = Vec::new();
    files(root, &mut all);
    all.sort();
    let mut buf = Vec::new();
    for p in &all {
        let rel = p.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/");
        let bytes = std::fs::read(p).unwrap();
        buf.extend_from_slice(rel.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        buf.extend_from_slice(&bytes);
    }
    let got = hex::encode(crx_tree::events::keccak256(&[&buf]));
    assert_eq!(got, VENDOR_KECCAK, "crates/vendor changed ({} files): re-vendor from the guest's owner, then repin", all.len());
}
