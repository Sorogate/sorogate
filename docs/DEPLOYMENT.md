# Public Testnet deployment

**This is a public development deployment on Stellar Testnet. It is not a production deployment.** Testnet is
reset from time to time and its tokens have no value, the contract has not been audited, and nothing here should be
used with real value. The machine-readable record is [`deployments/testnet.json`](deployments/testnet.json).

Label: **Live** means the fact was read from the network on the date shown. It was true then; run
[`verify`](#check-that-it-is-this-code) to see whether it still is.

| | |
| --- | --- |
| Network | Stellar Testnet (`Test SDF Network ; September 2015`), protocol 29 |
| Contract address | `CACR5H46E7VKEDJUWRLKZPJPQVPRHTRTJHZOMQLEIHMTXN4MW7O47YQW` ([explorer](https://stellar.expert/explorer/testnet/contract/CACR5H46E7VKEDJUWRLKZPJPQVPRHTRTJHZOMQLEIHMTXN4MW7O47YQW)) |
| Contract | `contracts/access-policy`, workspace version `0.0.0` (pre-release; the contract has no on-chain version function, so the WASM hash below is its identity) |
| WASM hash | `f702e9d267262ab3f9548fa62021b4c31bb5b5f4f3652ff713906dcd48906814`, 11,457 bytes. The code was fetched back from the network after deployment and compared with the local build byte for byte |
| WASM hash, ignoring the CLI version | `10170e707598f15fe8edf5bdf0817e1733a697496c5aa277931e23049ba1fce8`. The same file with the value of one metadata entry, `cliver`, blanked: see [below](#check-that-it-is-this-code) |
| Built from | commit `1e94264549a220a80d97347386042810f48e47ba`, `stellar contract build` with stellar-cli 27.1.0, rustc 1.96.0, soroban-sdk 28.0.0 |
| Deployment transactions | upload `e75b58c3bc2cb569c15dcd0a0acde7b1a22c40e9a38b9f1a377659fd17147c85`; create `ce7ca6247655cb0457cd44aa3c6b9a308a0793bd368a3929a070fd87d3971fba` (ledger 5,071,126) |
| Deployment date | 2026-10-07, 13:20:17 UTC (close time of the creation transaction) |
| Deployed by | `GBZEURYLVXFGQ5HOF6UFAP64SXN3Y74GPCONROASPJNSWQFH5PCZ6GAY`, a throwaway account whose secret key was never printed or saved |
| Admin / owner | **None.** The contract has no administrator, no owner and no upgrade path ([`SPEC.md`](../spec/SPEC.md) section 6). The deployer has no power over it, so discarding its key costs nothing |
| Specification | [`spec/SPEC.md`](../spec/SPEC.md), draft 0.1, as at the commit above |
| Policy versions | A policy starts at version 1 and its version goes up by one on every `update`. A deactivation does not change it |

On 2026-11-04 Testnet moved to protocol 30. The scripts were run again with this contract's exact bytes and with the current source, and
all steps and comparisons held: see [`evidence/testnet-protocol-30-2026-11-04.md`](evidence/testnet-protocol-30-2026-11-04.md). The table above is
the record of the deployment on 2026-10-07, at protocol 29, and is not rewritten.

## What that means

- **Policies on this contract belong to whoever created them.** The contract itself has no authority over any policy.
  Anyone can create a policy here, and only its owner can change or deactivate it.
- **Policies made for this repository's own evidence and examples are owned by discarded keys.** They can never be
  changed or deactivated. That is deliberate: it makes them stable examples, and it also means they must not be used for
  anything real.
- **A bug cannot be fixed in place.** A fixed contract is a new deployment with new policy ids that consumers have to
  move to. This deployment would stay as it is.
- **The example consumer pins this code.** [`Sorogate/example-consumer`](https://github.com/Sorogate/example-consumer) keeps the
  WASM hash above in a file, runs its tests against exactly that code, and checks every week that the network still holds it.
  There is **no formal interface version** yet: the assumption is the `evaluate(id, subject) -> Decision` interface in
  [`INTEGRATING.md`](INTEGRATING.md) and the reason codes in [`SPEC.md`](../spec/SPEC.md). A replacement deployment would have a new
  address and a new hash, and consumers would move to it by choice; this one would not change.
- **Testnet resets.** When Testnet is reset, this contract and everything on it disappears. This file would then be
  out of date until the contract is deployed again and the record rewritten.

## Check that it is this code

Anyone can check that the code on the network is the code in this repository. It needs no key and sends nothing:

```bash
stellar contract build
npm ci
npm run testnet:deployment -w @sorogate/sdk -- verify \
  --contract CACR5H46E7VKEDJUWRLKZPJPQVPRHTRTJHZOMQLEIHMTXN4MW7O47YQW \
  --wasm ../../target/wasm32v1-none/release/access_policy.wasm
```

It prints the size and hash of the code the network holds and of your build, and says `IDENTICAL`, `IDENTICAL EXCEPT THE CLI VERSION` or
`DIFFERENT`.

What this does and does not show:

- The build was **reproducible** in the one case it was tried: rebuilding at the deployment commit on the same machine
  with the same tools, on a different day, gave the same hash as every earlier recorded run (`f702e9d2…`).
- It was compared **across stellar-cli versions**, on one machine, with the same Rust compiler. The same sources built with
  28.1.0 (what CI uses) differ from the deployed bytes (built with 27.1.0) in **38 of 11,457 bytes**, all inside one
  contract-metadata entry, `cliver`, where the CLI records its own version and commit (`27.1.0#8e402ea…` against
  `28.1.0#c0f4d0d…`). Every other byte, code and metadata, is identical, and `verify` reports that as `IDENTICAL EXCEPT THE CLI
  VERSION`. The network identifies code by the hash of the whole file, so a different CLI gives a different code hash even
  for the same program. The "ignoring the CLI version" hash is a convenience for comparing builds, not a security
  property: anyone can write anything in that entry.
- It was compared **across machines**: on 2026-10-07 the CI check ran on GitHub's runner, with the same pinned Rust compiler
  (1.96.0) and stellar-cli 28.1.0, and reported the build as the deployed code, differing only in the CLI version. It was **not**
  compared across Rust compiler versions.
- **The lockfile is part of the identity.** Building the commit after the deployment's `Cargo.lock` changed (`serde_json` 1.0.151 to
  1.0.154, an automatic dependency update) gives a different hash, `0238d758…`, although `contracts/` has only new tests. Taking that
  one file back from the deployment commit gives `f702e9d2…` again. To check the deployed code, build **the deployment commit** (the
  `Built from` row above), not the latest `main`; see [`evidence/testnet-protocol-30-2026-11-04.md`](evidence/testnet-protocol-30-2026-11-04.md).
- **CI makes this comparison on every push** (`npm run check:deployed-wasm`, from `.github/workflows/ci.yml`). It passes when
  the build is the deployed code, or when the contract's sources have changed since the deployment commit (the deployment is
  then an older version, and the check says so). It **fails** when the sources have not changed but the code differs,
  which would mean the same source no longer builds to the same code. A change to the contract therefore passes CI, and
  is a reason to redeploy and update `deployments/testnet.json` when the change matters.

## Keeping it alive

A contract has two ledger entries that expire if nobody extends them: its **instance** and its **code** (and each policy in it is
a third). When one expires it is archived, not erased, and has to be restored before the contract can be used again. **A contract
that has just been deployed lives only about 7 days** (the network's minimum), until a write or a `bump` extends it, so anyone
deploying their own copy has to do one of them soon.

- At deployment both were extended to the network's practical maximum: ledger 8,071,127, about 173 days later
  (estimated 2027-03-30, at five seconds a ledger). Live, 2026-10-07.
- Anyone can extend them again, for any contract, with a funded account. This command uses a throwaway one:

  ```bash
  npm run testnet:deployment -w @sorogate/sdk -- extend --contract CACR5H46E7VKEDJUWRLKZPJPQVPRHTRTJHZOMQLEIHMTXN4MW7O47YQW
  ```

  It was run once, about a minute after deployment, and moved the end from ledger 8,071,127 to 8,071,148 (Live, 2026-10-07). That
  shows the command works. It does not show what happens to a contract that is close to expiry or already archived.
- **The policy contract's own `bump(id)` extends the policy, the contract's instance and its code**, each to 90 days from that
  ledger. Anyone can call it, and it needs a policy id that exists. A write (`create`, `update`, `set_active`) did the same to an
  entry with 7 days left; the contract's code tops an entry up only when less than 30 days remain, and that no-op case was not
  observed. A read such as `evaluate` extends nothing. Measured on Testnet, 2026-10-07, on a copy of the contract with its own code
  hash: [`evidence/testnet-ttl-2026-10-07.md`](evidence/testnet-ttl-2026-10-07.md). That is 90 days from each call, against about 173 from the `extend` command above, so `extend` is the longer
  way to keep the public deployment alive.

The network's configuration sets `max_entry_ttl` to 3,110,400 ledgers (read on 2026-10-07; see [`COSTS.md`](COSTS.md)). An
extension to exactly that number was refused as malformed and 3,000,000 was accepted, so the usable maximum is a little below the
configured one.

## Not this one

An earlier attempt the same day, `CAVNAUBPPSRCWVALWJIR6WMBL7ZTMAJXJLLXNTKSZBRIVAAZEZC3LFKQ`, deployed the same code but
stopped on a bug in the deployment script before its transactions could be recorded. It is unused and can be ignored.
