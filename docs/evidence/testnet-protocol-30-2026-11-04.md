# Testnet at protocol 30, 2026-11-04: six recorded runs

**Label: Recorded.** Testnet moved from protocol 29 to protocol 30 on 2026-11-03 at 17:00:05 UTC. These are manual runs
of three existing scripts on the protocol 30 Testnet, made the next day, twice: once with the **public deployment's
exact bytes** and once with the **current source**. They are not part of CI, because they use a public network. The
protocol 29 runs of 2026-10-07 and 2026-10-09 are unchanged and still describe protocol 29 only.

## Result

| Script | Deployed bytes (`f702e9d2...`) | Current source (`0238d758...`) |
| --- | --- | --- |
| `testnet-differential.ts` (8 policies, seed 1) | Same decision in **all 32 comparisons**. 0 disagreements, 0 unpaired, 0 codec problems. 11 of 32 allowed. | Same decision in all 25 paired comparisons. 0 disagreements, 7 unpaired, 0 codec problems. 9 of 25 allowed. |
| `testnet-gated-claim.ts` | **25 of 25 steps** as expected | 25 of 25 steps as expected |
| `testnet-sdk-writes.ts` | **11 of 11 steps** as expected | 11 of 11 steps as expected |

The gated-claim steps include both refusals of a claim signed under an older policy version, and the refusal of a claim
that a different account signed for someone else. The SDK-writes steps include its refusals and the intruder's update
failing with `TransactionFailedError`.

Raw output, deployed bytes (run 12:30 to 13:05 UTC):

- [`testnet-differential-2026-11-04-protocol-30-deployed-bytes.json`](testnet-differential-2026-11-04-protocol-30-deployed-bytes.json)
- [`testnet-gated-claim-2026-11-04-protocol-30-deployed-bytes.json`](testnet-gated-claim-2026-11-04-protocol-30-deployed-bytes.json)
- [`testnet-sdk-writes-2026-11-04-protocol-30-deployed-bytes.json`](testnet-sdk-writes-2026-11-04-protocol-30-deployed-bytes.json)

Raw output, current source (run 10:19 to 12:09 UTC):

- [`testnet-differential-2026-11-04-protocol-30.json`](testnet-differential-2026-11-04-protocol-30.json)
- [`testnet-gated-claim-2026-11-04-protocol-30.json`](testnet-gated-claim-2026-11-04-protocol-30.json)
- [`testnet-sdk-writes-2026-11-04-protocol-30.json`](testnet-sdk-writes-2026-11-04-protocol-30.json)

## How it was done

| | |
| --- | --- |
| Network | Stellar Testnet, protocol 30 (each script refuses any network that does not report the Testnet passphrase) |
| Scripts | From commit `2cdcfcb8b91c56bf4fe63d8d2d4cebffe8639e2a` (`main`) in both sets of runs |
| Tools | stellar-cli 27.1.0, rustc 1.96.0, `@stellar/stellar-sdk` 17.4.0, Node 22.22.1 under WSL (the package declares ^22.22.2 or higher; the scripts ran anyway) |
| Keys | Created by each script in memory and funded by friendbot. Never printed or saved. |
| Deployed-bytes runs | WASM built with `stellar contract build` at commit `1e94264` (the deployment commit). The run stops before any network call unless `access_policy.wasm` has sha256 `f702e9d2...906814`, the public deployment's hash. The reports record that hash. The test-token and consumer fixtures were built at the same commit. |
| Current-source runs | WASM built at `2cdcfcb`: `access_policy.wasm` sha256 `0238d758...c2c57b`. The fixtures had the same hashes as the `1e94264` build (`89146b0e...`, `143a6b5c...`). |
| Contracts | In every run each script deployed **its own copy** of the WASM. **The public deployment itself was not called** (`publicPolicyContractUsed: false` in the gated-claim reports). It ran the same bytes, on the same network, but it is a different contract instance. |

## Why there are two sets

`testnet-deployment.ts verify` said a build of current `main` is not the public deployment:

```
on the network: 11457 bytes, sha256 f702e9d267262ab3f9548fa62021b4c31bb5b5f4f3652ff713906dcd48906814
local file:     11457 bytes, sha256 0238d758b5746ec478684aa44810b5e1581e3190af132d7bc729d9fcf7c2c57b
ignoring the CLI version in the metadata: 10170e70...1fce8 and ab32a22c...69813
DIFFERENT: the deployed contract is not this build.
```

The size is the same and the difference is not the `cliver` entry. It was traced on a separate clone:

| Build | sha256 |
| --- | --- |
| Commit `1e94264` (the deployment commit), built in a different folder | `f702e9d2...906814`, the public deployment's hash |
| Commit `2cdcfcb` as committed | `0238d758...c2c57b` |
| Commit `2cdcfcb` with only `Cargo.lock` taken from `1e94264` | `f702e9d2...906814` |
| Commit `2cdcfcb` with `contracts/` from `1e94264` (new `Cargo.lock` kept) | `0238d758...c2c57b` |

So the public deployment's bytes are reproducible from its commit, the folder and the new tests do not matter, and the
difference comes from the `Cargo.lock` change between the two commits: `serde_json` 1.0.151 to 1.0.154 (an automatic
dependency update). Why that dependency ends up in the contract's bytes, and whether the two builds behave differently
apart from what these runs exercised, was not examined.

## What it shows

- With the public deployment's exact bytes, and also with the current source, contract deployment, `evaluate` through
  simulation, balance reads from other contracts, policy writes and the Stellar asset contract all behaved as expected
  on protocol 30, with no change to the contracts or the SDK.
- The contract and the TypeScript model agreed on every decision that was compared.

## What it does not show

- **8 policies, not the 30 policies and 120 comparisons of the protocol 29 run.** A first attempt at 30 policies was
  stopped because the public RPC server was slow at that time. Its partial log is not evidence and is not kept.
- **The current-source differential run had 7 unpaired comparisons** (the contract's answer and the model's snapshot did
  not come from the same ledger). The script treats this as a timing miss, not a disagreement, and they were not
  retried. The deployed-bytes run had none; RPC speed was better. Neither run is better evidence on that account.
- **The public contract instance was not called.** Same bytes, a different instance. Its stored state (the
  policies on it) was not read.
- Not run: `testnet-ttl.ts`, `testnet-costs.ts`, `testnet-sep50.ts`. Lifetimes, fees and the OpenZeppelin token behaviour
  on protocol 30 are **not measured**; [`COSTS.md`](../COSTS.md) and the TTL note still describe protocol 29.
- Fees and limits may differ between protocols. Nothing here compares costs.
- One day, one public RPC server, one seed. It found no problem; it cannot prove there is none.
