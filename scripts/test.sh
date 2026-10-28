#!/bin/sh
# Default test run. The vendored guest engine (crates/vendor) runs only the audit-tagged
# tests in FLOOR; VENDOR.tsv and crates/crx-tree/tests/vendor_pin.rs pin its bytes.
# CRX_TEST_LONG=1 runs every test of every member, the ELF-to-vkey and guest-execute checks included.
set -u
cd "$(dirname "$0")/.." || exit 2

if [ "${CRX_TEST_LONG:-0}" = 1 ]; then
  exec cargo test --workspace --locked --no-fail-fast
fi

# Audit-tagged tests of the copy (C-01, C-02, F-1, GS-02, GS-03, GS-05, H-2, M-02) that contracts
# prover/crx-risk-engine holds no twin of.
FLOOR="completeness::tests::c02_assert_no_trusted_settle_panics_on_some
completeness::tests::gs03_empty_boxes_with_real_positions_fails_binding
completeness::tests::gs05_due_position_without_proven_twap_panics
completeness::tests::gs05_completeness_exemptions_do_not_panic
handoff::tests::transfer_as_close_conserves_and_bounds_b
handoff::tests::gs02_valid_conserved_novation_passes_in_guest_check
handoff::tests::gs02_unconserved_minted_c_im_is_rejected_in_guest
handoff::tests::gs02_forged_party_is_rejected_in_guest
handoff::tests::gs02_quote_below_mark_breach_is_rejected_in_guest
handoff::tests::closeout_novation_happy_conserves_and_names_shortfall
margin::tests::c01_position_im_floor_sums_pushed_im
settlement::tests::proven_twap_binding_rejects_wrong_feed_or_close
settlement::tests::position_settlements_from_twaps_derives_only_bound_positions
settlement::tests::f1_two_same_binding_different_priced_twaps_rejected
settlement::tests::f1_single_and_distinct_key_twaps_accepted
settlement::tests::gs05_trusted_position_settlements_cannot_move_value
settlement::tests::m02_expo_rescales_only_the_settle_price"

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
