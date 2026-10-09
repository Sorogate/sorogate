# Contract against the TypeScript model, on Testnet, 2026-10-09: the allowed path

**Label: Recorded.** One manual run of `packages/sdk/scripts/testnet-differential.ts` after its generator was changed
to produce many more allowed decisions. Raw output: [`testnet-differential-2026-10-09.json`](testnet-differential-2026-10-09.json).
It is not part of CI, because it uses a public network. The earlier run, whose generator rarely produced an allowed
decision, is [`testnet-differential-2026-10-07.md`](testnet-differential-2026-10-07.md); it is unchanged.

## Result

The deployed contract and the TypeScript model gave the **same decision in all 120 comparisons**: 30 random
policies, each evaluated for 4 subjects. No disagreements, no cases that could not be read from one ledger, and no
case where a stored policy differed from what was sent (the codec round-tripped every policy through the contract).

**33 of the 120 decisions (27.5%) were allowed**, against 5 of 120 in the earlier run.

| Decision | Count |
| --- | --- |
| `BelowMinimum` | 39 |
| allowed (`None`) | 33 |
| `Inactive` | 24 |
| `BalanceUnavailable` | 20 |
| `AfterWindow` | 4 |

There was no `BeforeWindow` decision in this run (the earlier run had 9, and the shared vectors cover it).

## How it was done

| | |
| --- | --- |
| Network | Stellar Testnet, protocol 29 (the script refuses any network that does not report the Testnet passphrase) |
| Keys | Created by the script in memory and funded by friendbot. Never printed or saved. |
| Contract | A build of `access_policy.wasm` made by the person who ran this, sha256 `691d4f7f49f13de77436be76a8fd017e7cb67e570d6dfadd74cecfd6e6e5434f`. **This is not the public deployment's code** (`f702e9d2…`, see [`DEPLOYMENT.md`](../DEPLOYMENT.md)); the commit and the tool versions it was built with were not recorded, so this run says nothing about the public deployment's exact bytes. The earlier run did use that WASM. |
| Tokens | The `mock-token` test fixture (sha256 `a81ba48f…51bdd1`) in four behaviours (an `i128`, a `u32`, a `u64`, an error), the access-policy contract itself as a contract with no `balance` function, and **one real Stellar asset contract** with a funded holder |
| Subjects | `holder` (a real account with a trustline and a balance), `nonholder` (a real account with no trustline), and two accounts that do not exist on the network |
| Policies | 30, seed 1. For about 70% of them a subject is chosen first and one or two conditions are built that this subject satisfies where a suitable balance exists (a token or collection the subject holds, a minimum no larger than the balance, a time window around the ledger time). The rest are one to six random conditions, as in the earlier run. Some policies were updated once or twice, and some deactivated (the 24 `Inactive` decisions are 6 policies times 4 subjects). |
| Pairing | For each comparison the contract and the model were asked at the same moment, and the answer counted only if both came from the same ledger |

## What it shows that the other tests do not

- The **allowed path on the real network**: simulation, argument encoding, the contract's actual return values and
  balances read from real contracts, for 33 decisions where every condition held. Before this the live runs said
  little about it.
- The client's **one-ledger snapshots** working against a live RPC server, as in the earlier run. An account with no
  trustline and an account that does not exist both gave `BalanceUnavailable` in both implementations.

## What it does not show

- **The 27.5% is a property of the generator, not of real policies.** Seventy percent of the policies were built so that
  a chosen subject would pass them. Most random subjects evaluating a random policy still fail. The allowed path is
  also covered by the shared vectors and the offline random test, which run thousands of cases.
- **The public deployment's code was not the code under test** in this run (see the Contract row).
- No `BeforeWindow` decision occurred.
- Four subjects, all ordinary accounts. **Contract accounts (`C...`) as subjects** were not part of this run.
- Only one real token, and only the Stellar asset contract. **No third-party token** such as OpenZeppelin's, and
  nothing about the cost of a large token.
- **A token that exhausts its budget** (which aborts the contract's evaluation) was not tried.
- Simulation against execution: every answer came from simulations at one ledger. Whether a submitted
  transaction later behaves the same near a time-window edge is described in `spec/SPEC.md` section 4.2, not tested
  here.
- 30 policies is a small sample. It found no problem; it cannot prove there is none.
