//! The pinned guest ELF, run on the sp1 CPU executor (no prover, no network), takes the host's
//! frames for an epoch and commits the roots and scenario root the host predicts.
//! The debug-build executor runs over 60 s: the test runs under CRX_TEST_LONG=1 (release, audit, sp1 bump).

use crx_tree::frames::build_advance_frames;
use crx_tree::pv::decode_public_values;
use crx_tree::replay::{EpochInputs, TreeState};

#[test]
fn the_guest_commits_the_host_predicted_roots_for_an_empty_epoch() {
    if std::env::var("CRX_TEST_LONG").as_deref() != Ok("1") {
        eprintln!("the_guest_commits_the_host_predicted_roots_for_an_empty_epoch: runs under CRX_TEST_LONG=1");
        return;
    }
    let (table, table_root) = crx_tree::load_scenario_table(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/scenario-table.bin"
    )))
    .expect("committed scenario table loads");
    let state = TreeState::default();
    let inputs = EpochInputs {
        new_positions: vec![],
        proven_marks: vec![],
        proven_twaps: vec![],
        paused_pairs: vec![],
        proof_now: 1_761_609_600,
        domain_separator: [0x11; 32],
    };
    let adv = build_advance_frames(&state, &inputs, &table, state.accounts_root()).expect("frames build");
    let pv = prove_exec::execute(&adv.frames).expect("the guest executes the frames");
    let got = decode_public_values(&pv).expect("committed public values decode");
    assert_eq!(got.roots.root_prev, adv.root_prev, "rootPrev");
    assert_eq!(got.roots.root_new, adv.predicted_root, "rootNew");
    assert_eq!(got.roots.accounts_root_new, adv.predicted_accounts_root, "accountsRootNew");
    assert_eq!(got.scenario_root, table_root, "scenarioRoot");
    assert_eq!((got.proof_time, got.domain_separator), (inputs.proof_now, inputs.domain_separator));
}
