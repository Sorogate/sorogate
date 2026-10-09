# Contract against the TypeScript model, on Testnet, 2026-10-07

**Label: Recorded.** One manual run of `packages/sdk/scripts/testnet-differential.ts`. Raw output:
[`testnet-differential-2026-10-07.json`](testnet-differential-2026-10-07.json). It is not part of CI, because it
uses a public network.

## Result

The deployed contract and the TypeScript model gave the **same decision in all 120 comparisons**: 30 random
policies, each evaluated for 4 subjects. No disagreements, no cases that could not be read from one ledger, and no
case where a stored policy differed from what was sent (the codec round-tripped every policy through the contract).

| Decision | Count |
| --- | --- |
| `BalanceUnavailable` | 36 |
| `Inactive` | 28 |
| `BelowMinimum` | 29 |
| `AfterWindow` | 13 |
| `BeforeWindow` | 9 |
| allowed (`None`) | 5 |

## How it was done

| | |
| --- | --- |
| Network | Stellar Testnet, protocol 29 (the script refuses any network that does not report the Testnet passphrase) |
| Keys | Created by the script in memory and funded by friendbot. Never printed or saved. |
| Contract | `access_policy.wasm`, sha256 `f702e9d267262ab3f9548fa62021b4c31bb5b5f4f3652ff713906dcd48906814`, the same WASM as the earlier smoke run |
| Tokens | The `mock-token` test fixture (sha256 `89146b0e…922f3`) in four behaviours (an `i128`, a `u32`, a `u64`, an error), the access-policy contract itself as a contract with no `balance` function, and **one real Stellar asset contract** with a funded holder |
| Subjects | `holder` (a real account with a trustline and a balance), `nonholder` (a real account with no trustline), and two accounts that do not exist on the network |
| Policies | 30, seed 1, one to six conditions each. Some were updated (versions reached 2 and 3) and 7 were deactivated (28 `Inactive` decisions over 4 subjects). Minimums were chosen near each subject's real balance, and time windows near the ledger time. |
| Pairing | For each comparison the contract and the model were asked at the same moment, and the answer counted only if both came from the same ledger |

## What it shows that the other tests do not

- The **real network path**: simulation, argument encoding, the contract's actual return values, and balances read
  from real contracts, including a real asset contract. An account with no trustline and an account that does not
  exist both gave `BalanceUnavailable` in both implementations.
- The client's **one-ledger snapshots** working against a live RPC server.

## What it does not show

- **The allowed path is thin.** Only 5 of 120 decisions were allowed. The generator aims minimums at balances, but
  with random balances most policies fail somewhere. The allowed path is covered much better by the shared vectors
  and the random offline test, which run thousands of cases; do not read this run as evidence about it.
  A later run, with a generator changed to exercise this path, is in
  [`testnet-differential-2026-10-09.md`](testnet-differential-2026-10-09.md).
- Four subjects, all ordinary accounts. **Contract accounts (`C...`) as subjects** were not part of this run
  (a contract address was checked in the earlier smoke run).
- Only one real token, and only the Stellar asset contract. **No third-party token** such as OpenZeppelin's, and
  nothing about the cost of a large token.
- **A token that exhausts its budget** (which aborts the contract's evaluation) was not tried.
- Simulation against execution: every answer came from simulations at one ledger. Whether a submitted
  transaction later behaves the same near a time-window edge is described in `spec/SPEC.md` section 4.2, not tested
  here.
- 30 policies is a small sample. It found no problem; it cannot prove there is none.

## History

An earlier attempt of the same script agreed on its first 100 comparisons and then stopped, because the SDK read the
balances one after another and for one case that took longer than a ledger, so the contract's answer and the model's
snapshot never came from the same ledger. That was fixed in the SDK (balances are now read in parallel, with a test)
and in the script (a case that cannot be paired is counted and reported, never silently dropped). This report is
from the second run, which paired every case.
