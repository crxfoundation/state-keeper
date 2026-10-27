//! Every refusal the keeper answers, as a NAMED variant — the name is the diagnosis,
//! the message names the fix. Message strings are FROZEN output surface.
//! enforced by: crates/state-keeper/tests/cli.rs (each variant from its trigger)

use thiserror::Error;

/// Pre-flight and parity refusals, one variant per operator-fixable condition.
#[derive(Debug, Error)]
pub enum Gate {
    /// Fix: ship the scenario-table artifact of THIS generation, or repin chains.json.
    #[error("scenario table root mismatch: data/scenario-table.bin commits 0x{local}, chains.json pins 0x{pinned} — the core would reject every proof built on this table")]
    TableArtifactNotThisGeneration { local: String, pinned: String },
    /// Fix: rebuild against the generation the core answers, or repoint chains.json.
    #[error("the core answers a DIFFERENT vkey than this build is pinned to — do not advance")]
    VerifyVkeyDrift,
    /// Fix: replace data/scenario-table.bin with the on-chain generation's artifact.
    #[error("the committed scenario table does not match the on-chain scenarioRoot")]
    VerifyScenarioRootDrift,
    /// Fix: export PRIVATE_KEY (or put it in `.env`).
    #[error("PRIVATE_KEY is not set — the advance signs a real transaction")]
    PrivateKeyUnset,
    /// Fix: this build proves a different generation than the core verifies.
    #[error("core vkey 0x{chain} != pinned {pinned} — wrong generation, refusing")]
    AdvanceWrongGeneration { chain: String, pinned: String },
    /// Fix: replace data/scenario-table.bin with the on-chain generation's artifact.
    #[error("scenario table does not match the on-chain scenarioRoot — refusing to prove")]
    AdvanceScenarioRootDrift,
    /// The replayed event fold and the chain disagree — investigate before trusting either.
    #[error("REBUILD MISMATCH: the replayed tree does not reproduce the on-chain root")]
    RebuildRootMismatch,
    /// The replayed account registry and the chain disagree.
    #[error("REBUILD MISMATCH: the replayed registry does not reproduce the on-chain accountsRoot")]
    RebuildRegistryMismatch,
    /// Fix: replace data/scenario-table.bin with the on-chain generation's artifact.
    #[error("scenario table mismatch against the on-chain scenarioRoot")]
    RebuildScenarioRootMismatch,
}
