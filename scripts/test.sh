#!/bin/sh
# Default test run. The vendored guest engine (crates/vendor) runs only the audit-tagged
# tests in FLOOR; VENDOR.tsv and crates/crx-tree/tests/vendor_pin.rs pin its bytes.
# CRX_TEST_LONG=1 runs every test of every member, the ELF-to-vkey and guest-execute checks included.
set -u
cd "$(dirname "$0")/.." || exit 2

if [ "${CRX_TEST_LONG:-0}" = 1 ]; then
  exec cargo test --workspace --locked --no-fail-fast
fi

# Audit-tagged tests of the copy (F-1, GS-02, GS-03, GS-05, H-2) that contracts
# prover/crx-risk-engine holds no twin of.
FLOOR="completeness::tests::gs03_empty_boxes_with_real_positions_fails_binding
completeness::tests::gs05_due_position_without_proven_twap_panics
completeness::tests::gs05_completeness_exemptions_do_not_panic
handoff::tests::transfer_as_close_conserves_and_bounds_b
handoff::tests::gs02_valid_conserved_novation_passes_in_guest_check
handoff::tests::gs02_unconserved_minted_c_im_is_rejected_in_guest
handoff::tests::gs02_forged_party_is_rejected_in_guest
handoff::tests::gs02_quote_below_mark_breach_is_rejected_in_guest
handoff::tests::closeout_novation_happy_conserves_and_names_shortfall
settlement::tests::proven_twap_binding_rejects_wrong_feed_or_close
settlement::tests::position_settlements_from_twaps_derives_only_bound_positions
settlement::tests::f1_two_same_binding_different_priced_twaps_rejected
settlement::tests::f1_single_and_distinct_key_twaps_accepted
settlement::tests::gs05_trusted_position_settlements_cannot_move_value"

rc=0
cargo test --workspace --locked --no-fail-fast \
  --exclude crxvm --exclude im-calibration --exclude im-crypto \
  --exclude im-math --exclude im-state --exclude im-types || rc=1

want=$(printf '%s\n' "$FLOOR" | grep -c .)
# shellcheck disable=SC2086
have=$(cargo test -q --locked -p crxvm --lib -- --list --exact $FLOOR 2>/dev/null | grep -c ': test$')
if [ "$have" != "$want" ]; then
  echo "scripts/test.sh: $have of $want crxvm floor tests found" >&2
  rc=1
fi
# shellcheck disable=SC2086
cargo test --locked -p crxvm --lib -- --exact $FLOOR || rc=1
exit $rc
