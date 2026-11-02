# Threat model

Status: **early, not audited, Testnet only.** One maintainer, and the contract and the TypeScript model were written
by the same person from the same specification, so a mistake in the specification itself would be in both.

This page says what the project is meant to protect, what it assumes, what could go wrong, and what it deliberately
does not protect. Each claim says what stands behind it, in the words defined in [`EVIDENCE.md`](EVIDENCE.md): **Recorded** (a
run on Testnet, kept in [`docs/evidence`](evidence)), **Unit-tested** (a test in this repository), **Documented** (Stellar's own
documentation), or **Reasoning** (follows from the design, not checked by a run). Rules are in [`spec/SPEC.md`](../spec/SPEC.md).

## What is protected

1. **The decision is the one the specification describes** for a given stored policy and chain state.
2. **Only the policy's owner can change it, deactivate it, or reactivate it.**
3. **A consumer can tell that the rules behind a policy changed** (the version in every decision).
4. **A failure to read a balance is a denial, never a pass.**

## What is not protected

Read this part first if you are deciding whether to depend on the primitive.

- **Identity.** `evaluate` answers for any address it is given. It does not prove who is asking. Authenticating the
  subject is the consumer's job ([`docs/INTEGRATING.md`](INTEGRATING.md)).
- **Uniqueness of a person.** A balance is a snapshot, not an identity. The same tokens can satisfy a policy for one
  address, move, and satisfy it for another. A flash loan can satisfy it for one transaction.
- **A malicious or mistaken policy owner.** Consumers that follow a policy trust its owner, who can change the rules at
  any time. A consumer can pin a version instead.
- **The tokens a policy names.** They are other people's contracts, chosen by the policy owner. A token can burn its whole
  budget (which aborts the calling transaction), and a token that is upgradable can change what it reports without the
  policy's version changing.
- **Secrecy.** Checking an address in a web page does not hide anything from someone who edits the page.
- **Anything on Mainnet.** The project has not been deployed there and should not be used with real value.

## Who is involved

| Actor | What they control |
| --- | --- |
| Policy owner | One policy: its conditions and whether it is active |
| Subject | Their own address and its balances |
| Consumer author | A contract that calls `evaluate` and acts on the answer |
| Token author | A token or collection a policy names. Chosen by the policy owner, not by us |
| Anyone | Can call `evaluate`, `get` and `bump` on any policy, and create their own policies |
| RPC provider | What an app reads when it simulates a call |

## Threats

### The policy contract

| Threat | What stops it | Evidence | What remains |
| --- | --- | --- | --- |
| Someone other than the owner changes or deactivates a policy | `update` and `set_active` require the stored owner's authorization | **Unit-tested** (the rejection is a unit test, and removing the check makes it fail) | If the owner's key is lost, the policy can never change. If it is stolen, the thief controls the policy. There is no ownership transfer, deletion or administrator, by design |
| The contract gives a wrong decision | One specification, 70 shared vectors, thousands of random cases compared against an independently written TypeScript model on every push, and a comparison on Testnet | **Unit-tested**, and **Recorded** ([differential run](evidence/testnet-differential-2026-10-07.md): 120 of 120 agree) | Both implementations come from one author and one specification. No external review, no formal verification, no fuzzing beyond the random comparison |
| A policy names an account address as a token, so evaluating it would abort the transaction | `create` and `update` refuse any address that is not a deployed contract | **Recorded** ([an account address aborts the whole call](evidence/spike-2026-10-06.md)); **Unit-tested** | A contract cannot later become an account, so `evaluate` does not check again. Reasoning, not a run |
| A token raises an error, panics, has no `balance` function, or returns the wrong type | The condition fails with `BalanceUnavailable` | **Recorded** and **Unit-tested** | For a classic asset this is also the normal answer for "does not hold it" (no trustline), so a denial reason must not be read as an outage |
| A token burns its whole budget | Nothing can catch it: the calling transaction aborts. The owner chooses the tokens, a policy has at most 8 conditions, and a consumer should only use policies whose owner it trusts | **Recorded** (the whole call fails with a budget error) | A hostile owner can make a policy that makes every transaction using it fail. This is a denial of service on that policy's users |
| A token changes what it reports later (an upgradable token) | The version covers the policy, not the tokens | **Reasoning** | A decision can change without any version change. Only trust tokens you trust |
| A token calls back into the policy contract | `evaluate` writes nothing, so re-entry has nothing to corrupt, and the host refused the re-entry in the test | **Recorded** (the call back did not succeed; the exact error code was not recorded) | |
| An app shows a stale answer: a simulation sees the last closed ledger, and the real transaction runs a few seconds later | The answer when the transaction executes is the authoritative one. Off-chain answers are advisory | **Recorded** ([timestamps](evidence/spike-2026-10-06.md)) | Near the edge of a time window, an off-chain "allowed" can be wrong by a ledger |
| A policy is archived because nobody extended its lifetime | Writes and `bump` extend the policy, the contract instance and its code to 90 days. Anyone can call `bump`. A transaction that uses an archived policy must restore it first, and the RPC simulation adds that | **Unit-tested** (the lifetime is extended; a stranger can bump); **Recorded** ([on Testnet a write and a stranger's `bump` moved the instance, the code and the policy to 90 days, and a read moved none](evidence/testnet-ttl-2026-10-07.md)); **Documented** (restoring) | Availability, not correctness: an archived policy costs a restore and some delay. Nobody is paid to call `bump` |
| Someone floods the contract with policies | Creating one costs the creator fees and rent | **Reasoning** | The contract has no limit, quota or fee of its own |

### A consumer (the reference one, `gated-claim`)

| Threat | What stops it | Evidence | What remains |
| --- | --- | --- | --- |
| Claiming on behalf of an address that did not authorize it | The consumer calls `subject.require_auth()` first | **Unit-tested** (including a third party authorizing for someone else); **Recorded** ([a claim signed by someone else, for an address the policy allowed, was applied on Testnet and failed with a host authorization error, `auth: invalid_action`; the same address then claimed for itself and was paid](evidence/testnet-gated-claim-2026-10-07.md)) | A consumer that omits this line is open to anyone. The guide says so, but nothing can force a consumer to follow it |
| Claiming twice | A per-address record, written before payment, extended for 90 days | **Unit-tested**, **Recorded** | A forgotten record is archived, not erased, and a transaction touching an archived key must restore it first (**Documented**), so a record cannot be recreated by a second claim |
| A failed payment leaving a claim recorded | The record and the payment are in one call, so a failed payment reverts both | **Unit-tested** (an empty pool) | |
| The policy owner changes the rules under the consumer | An optional pinned version. The pin is checked before the answer | **Unit-tested**, **Recorded** (claims signed before the change were applied after it and failed with `BelowMinimum` and `PolicyChanged`; [evidence](evidence/testnet-gated-claim-2026-10-07.md)) | An unpinned consumer follows its owner on purpose |
| The pool is drained by recycling the same tokens across many addresses | Nothing. A balance is not an identity | **Reasoning** | The most a demonstration loses is its pool. Fund it only with a test asset; it has no withdrawal |

### The SDK and the model

| Threat | What stops it | Evidence | What remains |
| --- | --- | --- | --- |
| The model says "allowed" and the contract says "denied" | The model is tested against the same vectors and against the contract; the contract is documented as the authority | **Unit-tested**, **Recorded** | A preview can differ near a window edge or if a token misbehaves in a way the model cannot see (a budget-burning token is `unavailable` in the model and an abort in the contract) |
| A snapshot mixes balances from different ledgers | `fetchSnapshot` reads everything from one ledger and reads again if the ledger moves | **Unit-tested** (including a ledger that never holds still) | |
| The SDK reads malformed data and trusts it | The decoders refuse anything that does not match the contract's shapes | **Unit-tested**, including against real recorded contract output | |

### Not built yet

There is no web application. When there is one, these will need their own entries: gating content in a browser is not
secrecy (only a server that holds the content can enforce it), a signed message needs a nonce and an expiry or it can be
replayed (SEP-53 defines neither), message signing works for classic accounts only, and not every wallet supports it.

### Supply chain and operation

| Threat | What stops it | What remains |
| --- | --- | --- |
| A compromised dependency | Lockfiles for Rust and npm, a pinned Rust toolchain, weekly Dependabot updates, an advisory scan (`cargo audit` and `npm audit`, [`audit.yml`](../.github/workflows/audit.yml)) on every push to `main`, weekly, and on any pull request that changes a dependency file, and every GitHub Action pinned to a commit hash with its tag in a comment (Dependabot updates them) | A scan only finds advisories that have been published. A vulnerability fails the scan job, except the one listed below that the scan is told to ignore; an "unmaintained" warning on a Rust crate does not. One warning is known: `paste` 1.0.15 (RUSTSEC-2024-0436, unmaintained, no vulnerability) comes in through the Soroban SDK's host crates (`soroban-env-host`, `ark-ff`) and is not in the dependency graph of the contract built for `wasm32v1-none`. It is accepted until the SDK moves off it. One vulnerability is known and ignored on purpose, in [`.cargo/audit.toml`](../.cargo/audit.toml): `crypto-bigint` 0.5.5 (RUSTSEC-2026-0387, timing variability in operations meant to be constant-time, fixed only in 0.7.6 or later). It comes in through `soroban-env-host`, the Soroban host that tests run on, by way of `k256`, `p256` and `elliptic-curve` 0.13. It cannot be fixed by updating, because every `soroban-env-host` release up to 30.0.0 depends on `elliptic-curve` 0.13. It is not in the contract: `cargo tree -i crypto-bigint@0.5.5 -e normal --target wasm32v1-none` prints "nothing to print", no curve or signature crate is in the contract's build at all, and no contract here calls the host's signature or curve functions. It could matter only where a secret value passes through that host crate, and nothing here does that. This is **Reasoning** from the dependency graph on 2026-11-02, and no test enforces it. An ignored advisory is not reported again, so the entry is removed when a `soroban-env-host` release moves off `crypto-bigint` 0.5. The scan installs `cargo-audit` from crates.io, at a pinned version. The scan job is not a required check |
| The deployed contract is not the code in this repository | `stellar contract build` is the only supported build, recorded runs list the WASM hash, and `testnet-deployment.ts verify` compares the code the network holds for a contract with a local build ([`DEPLOYMENT.md`](DEPLOYMENT.md)) | CI rebuilds the contract on every push and compares it with the deployment record: it passes if the build is the deployed code (differing at most in the CLI version stamped in the metadata), or if the contract's sources have changed since the deployment, and fails if the sources are unchanged but the code differs. A rebuild with another stellar-cli version differed in the 38 bytes of that stamp and nowhere else. On 2026-10-07 CI's runner, a different machine with the same pinned compiler, reproduced it; it has not been compared across
compiler versions. The check is a convenience: the hash of the whole file is what identifies a contract |
| A bug found after deployment | There is no upgrade path and no administrator, on purpose. A fix is a new deployment with new policy ids that consumers must move to | Consumers are tied to a particular deployment |
| Keys | Testnet runs use throwaway keys created in memory and never printed or saved. Nothing in the repository holds a key | There is no Mainnet key management because there is no Mainnet use |

## What has and has not been checked

Checked: the unit tests of each contract; the shared vectors against both implementations; random policies against both
implementations on every push; the policy contract, the TypeScript model, the SDK client and the reference consumer
against deliberately broken versions of their code (an injected bug that no test caught was treated as a gap and
closed, or recorded as equivalent; the test-token fixture was not checked this way); recorded runs on Testnet for the
contract, the comparison with the model, and the consumer; the cost of evaluating policies over real tokens
([`COSTS.md`](COSTS.md)).

Not checked: any external security review; formal verification; fuzzing beyond the random comparison; contract accounts
(`C...`) as subjects on a live network; a collection or token that someone else deployed and operates (OpenZeppelin's SEP-50 example was deployed and checked by us, [evidence](evidence/testnet-sep50-2026-10-07.md)); behaviour
when a policy or claim record is actually archived; behaviour under sustained load.

## Reporting a problem

See [`SECURITY.md`](../SECURITY.md). Please do not describe a vulnerability in a public issue.
