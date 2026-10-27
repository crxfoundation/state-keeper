//! The CLI and the chain scan against a loopback JSON-RPC node: no public RPC, no prover, no send.
//! Each run gets a lane directory with its own chains.json and the committed scenario table.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

use alloy::primitives::{Address, B256};
use alloy::sol_types::{SolCall, SolEvent};
use crx_tree::events::{keccak256, openLockCall, TermsOpened};
use crx_tree::replay::TreeState;
use serde_json::{json, Value};

/// Public anvil test account #0: it holds nothing and no call here reaches a send.
const ANVIL_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const CORE: &str = "0x00000000000000000000000000000000000000c0";
const LENS: &str = "0x00000000000000000000000000000000000000e1";
const VKEY: [u8; 32] = [0x0a; 32];

#[derive(Clone)]
struct Node {
    root: [u8; 32],
    accounts_root: [u8; 32],
    scenario_root: [u8; 32],
    vkey: [u8; 32],
    head: u64,
    logs: Vec<Value>,
}

fn table() -> (PathBuf, [u8; 32]) {
    let p = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/scenario-table.bin"));
    let (_, root) = crx_tree::load_scenario_table(&p).expect("committed table loads");
    (p, root)
}

/// The node a lane with no events holds: empty tree, empty registry, the committed table.
fn empty_lane() -> Node {
    let empty = TreeState::default();
    Node { root: empty.root(), accounts_root: empty.accounts_root(), scenario_root: table().1, vkey: VKEY, head: 100, logs: vec![] }
}

fn selector(sig: &str) -> String {
    hex::encode(&keccak256(&[sig.as_bytes()])[..4])
}

fn word(b: [u8; 32]) -> String {
    format!("0x{}", hex::encode(b))
}

/// Serves JSON-RPC on 127.0.0.1 until the test process ends; returns the URL and every request.
fn serve(node: Node) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if r.read_line(&mut line).unwrap_or(0) == 0 {
                continue;
            }
            let mut len = 0usize;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h == "\r\n" || h.is_empty() {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
            }
            let mut body = vec![0u8; len];
            r.read_exact(&mut body).unwrap();
            let req: Value = serde_json::from_slice(&body).unwrap();
            let answer = answer(&node, &req);
            log.lock().unwrap().push(req);
            let text = answer.to_string();
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                text.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(text.as_bytes());
        }
    });
    (url, seen)
}

fn answer(node: &Node, req: &Value) -> Value {
    let ok = |result: Value| json!({"jsonrpc": "2.0", "id": req["id"], "result": result});
    match req["method"].as_str().unwrap_or("") {
        "eth_chainId" => ok(json!("0x7a69")),
        "eth_blockNumber" => ok(json!(format!("0x{:x}", node.head))),
        "eth_getLogs" => ok(Value::Array(node.logs.clone())),
        "eth_call" => {
            let call = &req["params"][0];
            let data = call["input"].as_str().or_else(|| call["data"].as_str()).unwrap_or("");
            let sel = data.trim_start_matches("0x").get(..8).unwrap_or("").to_string();
            let mut at = [0u8; 32];
            at[24..].copy_from_slice(&1_761_000_000u64.to_be_bytes());
            let w = if sel == selector("root()") {
                node.root
            } else if sel == selector("rootAt()") {
                at
            } else if sel == selector("accountsRoot()") {
                node.accounts_root
            } else if sel == selector("scenarioRoot()") {
                node.scenario_root
            } else if sel == selector("DOMAIN()") {
                [0x0d; 32]
            } else if sel == selector("imVkey()") {
                node.vkey
            } else {
                return json!({"jsonrpc": "2.0", "id": req["id"], "error": {"code": 3, "message": "execution reverted"}});
            };
            ok(json!(word(w)))
        }
        _ => json!({"jsonrpc": "2.0", "id": req["id"], "error": {"code": -32601, "message": "method not found"}}),
    }
}

/// A lane dir: chains.json (lane `t`, pinned to `pin`, RPC `url`) and data/scenario-table.bin.
fn lane_dir(name: &str, url: &str, pin: [u8; 32]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("state-keeper-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::copy(table().0, dir.join("data/scenario-table.bin")).unwrap();
    let lanes = json!({"lanes": [{
        "name": "t", "chain_id": 31337, "core": CORE, "lens": LENS, "deploy_block": 0,
        "vkey": word(VKEY), "scenario_root": word(pin), "rpc_url": url
    }]});
    std::fs::write(dir.join("chains.json"), lanes.to_string()).unwrap();
    dir
}

fn run(dir: &Path, args: &[&str], key: Option<&str>) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_state-keeper"));
    c.current_dir(dir).args(args).env_remove("RPC_URL").env_remove("PRIVATE_KEY").env_remove("NETWORK_PRIVATE_KEY");
    if let Some(k) = key {
        c.env("PRIVATE_KEY", k);
    }
    c.output().expect("the binary runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn an_unknown_verb_prints_the_usage_and_exits_2() {
    let o = run(&std::env::temp_dir(), &["fold"], None);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).starts_with("usage: state-keeper <rebuild|advance|verify>"));
}

#[test]
fn a_table_off_the_lane_pin_is_refused_before_any_chain_read() {
    let (url, seen) = serve(empty_lane());
    let dir = lane_dir("pin", &url, [0x11; 32]);
    let o = run(&dir, &["verify", "--chain", "t"], None);
    assert!(!o.status.success());
    assert!(stderr(&o).contains(&format!("chains.json pins 0x{}", hex::encode([0x11u8; 32]))), "{}", stderr(&o));
    assert!(seen.lock().unwrap().is_empty(), "no RPC call before the table gate");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn advance_without_a_private_key_is_refused_before_any_chain_read() {
    let (url, seen) = serve(empty_lane());
    let dir = lane_dir("nokey", &url, table().1);
    let o = run(&dir, &["advance", "--chain", "t"], None);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("PRIVATE_KEY is not set"), "{}", stderr(&o));
    assert!(seen.lock().unwrap().is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn verify_passes_when_the_core_answers_both_pins() {
    let (url, _) = serve(empty_lane());
    let dir = lane_dir("verify-ok", &url, table().1);
    let o = run(&dir, &["verify", "--chain", "t"], None);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stdout(&o).contains("OK — vkey and scenario root both match the pins."));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn verify_refuses_a_core_on_another_vkey_or_scenario_root() {
    for (name, node, want) in [
        ("verify-vkey", Node { vkey: [0x0b; 32], ..empty_lane() }, "the core answers a DIFFERENT vkey"),
        ("verify-scen", Node { scenario_root: [0x0c; 32], ..empty_lane() }, "does not match the on-chain scenarioRoot"),
    ] {
        let (url, _) = serve(node);
        let dir = lane_dir(name, &url, table().1);
        let o = run(&dir, &["verify", "--chain", "t"], None);
        assert!(!o.status.success());
        assert!(stderr(&o).contains(want), "{name}: {}", stderr(&o));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn rebuild_of_a_lane_with_no_events_matches_the_empty_tree() {
    let (url, _) = serve(empty_lane());
    let dir = lane_dir("rebuild-ok", &url, table().1);
    let o = run(&dir, &["rebuild", "--chain", "t"], None);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stdout(&o).contains("MATCH"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rebuild_names_the_root_the_events_do_not_reproduce() {
    for (name, node, want) in [
        ("rebuild-root", Node { root: [7; 32], ..empty_lane() }, "the replayed tree does not reproduce the on-chain root"),
        ("rebuild-reg", Node { accounts_root: [7; 32], ..empty_lane() }, "the replayed registry does not reproduce"),
        ("rebuild-scen", Node { scenario_root: [7; 32], ..empty_lane() }, "scenario table mismatch against the on-chain scenarioRoot"),
    ] {
        let (url, _) = serve(node);
        let dir = lane_dir(name, &url, table().1);
        let o = run(&dir, &["rebuild", "--chain", "t"], None);
        assert!(!o.status.success());
        assert!(stderr(&o).contains(want), "{name}: {}", stderr(&o));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn advance_refuses_another_generation_before_scanning_or_proving() {
    for (name, node, want) in [
        ("adv-vkey", Node { vkey: [0x0b; 32], ..empty_lane() }, "wrong generation, refusing"),
        ("adv-scen", Node { scenario_root: [0x0c; 32], ..empty_lane() }, "refusing to prove"),
    ] {
        let (url, seen) = serve(node);
        let dir = lane_dir(name, &url, table().1);
        let o = run(&dir, &["advance", "--chain", "t"], Some(ANVIL_KEY));
        assert!(!o.status.success());
        assert!(stderr(&o).contains(want), "{name}: {}", stderr(&o));
        assert!(seen.lock().unwrap().iter().all(|r| r["method"] != "eth_getLogs"), "{name}: no scan");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn advance_with_nothing_to_fold_exits_without_proving() {
    let (url, seen) = serve(empty_lane());
    let dir = lane_dir("adv-noop", &url, table().1);
    let o = run(&dir, &["advance", "--chain", "t"], Some(ANVIL_KEY));
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stdout(&o).contains("nothing to advance"));
    let methods: Vec<String> = seen.lock().unwrap().iter().map(|r| r["method"].as_str().unwrap_or("").to_string()).collect();
    assert!(!methods.iter().any(|m| m == "eth_sendRawTransaction" || m == "eth_estimateGas"), "{methods:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

fn provider(url: &str) -> impl alloy::providers::Provider {
    alloy::providers::ProviderBuilder::new().connect_http(url.parse().unwrap())
}

#[tokio::test]
async fn the_scan_reads_logs_in_90000_block_chunks_up_to_head() {
    let (url, seen) = serve(Node { head: 200_000, ..empty_lane() });
    let core: Address = CORE.parse().unwrap();
    let h = crx_tree::scan::scan(&provider(&url), core, 5).await.unwrap();
    assert_eq!(h.head, 200_000);
    let ranges: Vec<(String, String)> = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["method"] == "eth_getLogs")
        .map(|r| (r["params"][0]["fromBlock"].as_str().unwrap().to_string(), r["params"][0]["toBlock"].as_str().unwrap().to_string()))
        .collect();
    let want = [(5u64, 90_004u64), (90_005, 180_004), (180_005, 200_000)];
    let want: Vec<(String, String)> = want.iter().map(|(a, b)| (format!("0x{a:x}"), format!("0x{b:x}"))).collect();
    assert_eq!(ranges, want);
}

#[tokio::test]
async fn the_scan_refuses_an_open_event_whose_id_is_not_the_terms_hash() {
    let raw = include_str!("../../crx-tree/tests/fixtures/celo/open-1-calldata.hex");
    let calldata = hex::decode(raw.trim().trim_start_matches("0x")).unwrap();
    let terms = openLockCall::abi_decode(&calldata).unwrap().t;
    let core: Address = CORE.parse().unwrap();
    let event = TermsOpened { id: B256::repeat_byte(0xee), terms };
    let log = alloy::rpc::types::Log {
        inner: alloy::primitives::Log { address: core, data: event.encode_log_data() },
        block_hash: Some(B256::repeat_byte(2)),
        block_number: Some(50),
        block_timestamp: None,
        transaction_hash: Some(B256::repeat_byte(1)),
        transaction_index: Some(0),
        log_index: Some(0),
        removed: false,
    };
    let (url, _) = serve(Node { logs: vec![serde_json::to_value(&log).unwrap()], ..empty_lane() });
    let e = crx_tree::scan::scan(&provider(&url), core, 0).await.err().expect("a forged id must not scan");
    assert!(e.to_string().starts_with("TermsOpened id mismatch at block 50"), "{e}");
}
