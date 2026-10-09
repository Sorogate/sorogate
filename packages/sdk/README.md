# @sorogate/sdk

TypeScript for working with Sorogate access policies. Not published yet. **Early, Testnet only, not audited** (see the
[root README](../../README.md)): do not use it with real value.

It has two halves, and the contract is the authority:

- **A model of the rules** (`validateConditions`, `evaluate`, `decodeTokenBalance`, `decodeNftBalance`). Pure
  functions with no network. They follow [`spec/SPEC.md`](../../spec/SPEC.md) and are tested against the same
  [vectors](../../spec/vectors) as the contract.
- **A read-only client** (`evaluateOnChain`, `getPolicy`, `readBalance`, `fetchSnapshot`). It simulates calls through
  a Soroban RPC server. Nothing is signed or submitted.

## Using it today

It is not on npm yet, so use it from this repository. Build it, pack it, and install the tarball in your own project:

```bash
git clone https://github.com/Sorogate/sorogate.git && cd sorogate
npm ci
npm run build -w @sorogate/sdk
npm pack -w @sorogate/sdk        # writes sorogate-sdk-0.0.0.tgz
# then, in your project:
npm install /path/to/sorogate-sdk-0.0.0.tgz
```

`scripts/check-pack.sh` does exactly this in an empty project on every push and imports the result, so a change that breaks the package
fails CI. Two entry points are installed: `@sorogate/sdk` (everything) and `@sorogate/sdk/model` (the parts that need no network
and no Stellar library, for a web page).

## Ask the contract

```ts
import { rpc } from '@stellar/stellar-sdk';
import { evaluateOnChain } from '@sorogate/sdk';

const context = {
  rpc: new rpc.Server('https://soroban-testnet.stellar.org'),
  networkPassphrase: 'Test SDF Network ; September 2015',
  source: 'G...', // any account that exists on Testnet (friendbot can make one); it only sources the simulated transaction
};

const { decision, ledgerSequence } = await evaluateOnChain(context, {
  contractId: 'CACR5H46E7VKEDJUWRLKZPJPQVPRHTRTJHZOMQLEIHMTXN4MW7O47YQW', // the public Testnet deployment
  policyId: 13n, // an example policy on it: hold at least 10,000 XLM
  subject: 'G...', // the address to check
});
// decision = { allowed, version, failedIndex, reason }
```

`evaluateOnChain` is the answer that counts. It does **not** prove the caller controls `subject`; see the
specification, section 8.

The contract and the policy above are on a development deployment ([`docs/DEPLOYMENT.md`](../../docs/DEPLOYMENT.md)) and last only
until Testnet is next reset. Policy 13 is one the example repository's demo created, owned by a discarded key, so it never changes.

## Work it out locally

```ts
import { evaluate, fetchSnapshot, getPolicy } from '@sorogate/sdk';

const { policy } = await getPolicy(context, { contractId, policyId: 1n });
const { snapshot } = await fetchSnapshot(context, { conditions: policy.conditions, subject });
const decision = evaluate(policy, snapshot);
```

`fetchSnapshot` reads the ledger time and every balance from **one ledger** (it reads again if the network moves
on), because a balance and a time from different ledgers describe no moment that existed. Use this path to preview
a policy or explain a denial; use `evaluateOnChain` when the answer matters.

Two things to know:

- A simulation runs against the latest closed ledger, and the transaction it predicts is applied in a later one.
  Near the edge of a time window, the two can differ.
- A token that exhausts its whole budget makes the contract abort without a decision. `readBalance` reports such
  a token as `unavailable`.

The functions that need no network (`validateConditions`, `evaluate`, `toBaseUnits`, `fromBaseUnits`,
`describeCondition`, `explainDecision` and the types) are also available from `@sorogate/sdk/model`, which does not
import the Stellar SDK. Use it in a web page that only works a policy out locally, so the page does not ship a
library it never calls. It exports the same code as the package root, not a copy.

## Amounts

A policy's `min` is in a token's **base units**: with 7 decimals, one whole token is 10,000,000. Typing a display amount
where base units are expected gives a minimum that is wrong by a factor of ten to the number of decimals. Convert with
these, which use exact text and never floating point:

```ts
import { fromBaseUnits, readDecimals, toBaseUnits } from '@sorogate/sdk';

const decimals = await readDecimals(context, tokenAddress); // calls the token's decimals()
const min = toBaseUnits('12.5', decimals);                  // 125000000n for 7 decimals
fromBaseUnits(min, decimals);                               // '12.5'
```

`toBaseUnits` refuses an amount with more decimal places than the token has, instead of rounding it, and refuses
anything that is not plain digits with an optional decimal part.

## Showing a decision to a person

```ts
import { explainDecision } from '@sorogate/sdk';

explainDecision(decision, policy.conditions, { decimals: { [tokenAddress]: 7 } });
// "Denied: condition 2 of 2 is not met: holds at least 150 of token CBUXA…GEFX. The balance is lower (policy version 1)."
```

`describeCondition` describes one condition. Neither decides anything; they only describe what the contract or the model
decided.

## Changing a policy

A page that lets the owner change a policy builds the transaction **unsigned**, asks a wallet to sign it, and sends the
result. Nothing in the SDK holds a key.

```ts
import { prepareCreatePolicy, submitSigned } from '@sorogate/sdk';

const writes = { rpc, networkPassphrase: 'Test SDF Network ; September 2015' };
const prepared = await prepareCreatePolicy(writes, { contractId, owner: ownerAddress, conditions });
const signedXdr = await wallet.sign(prepared.xdr);                   // your wallet code
const { returnValue, hash } = await submitSigned(writes, signedXdr); // returnValue is the new policy id
```

`prepareUpdatePolicy` and `prepareSetActive` work the same way. The owner must be the transaction's source account, which is
how one signature satisfies the contract's authorization check. A policy the SDK can already see is invalid is refused
before anything is sent (`InvalidPolicyError`); one the contract refuses is reported as `ContractCallError`. The fee
bid is the network minimum and the transaction is valid for five minutes, so a person has time to sign; both can be
changed.

## Credentials

Credentials are not a condition of a policy, and nothing here checks one. `CredentialSource` is only a type that an app or
consumer can implement to plug a verifier in. [`docs/CREDENTIALS.md`](../../docs/CREDENTIALS.md) explains why.

## Compare with the contract on Testnet (manual)

`scripts/testnet-differential.ts` deploys the contract and the test-token fixtures to Testnet with throwaway keys, creates
random policies, and checks that `evaluateOnChain` and the model agree for every one. It takes a few minutes, talks to a
public network, and is not part of CI. The results of two runs are in
[`docs/evidence`](../../docs/evidence/testnet-differential-2026-10-07.md) and, after the generator was changed to
exercise the allowed path, [`2026-10-09`](../../docs/evidence/testnet-differential-2026-10-09.md).

```bash
stellar contract build
npm run testnet:differential -w @sorogate/sdk -- --wasm-dir <the folder with access_policy.wasm and mock_token.wasm> --policies 30 --seed 1
```

`npm run testnet:sdk-writes -w @sorogate/sdk -- --wasm-dir <the folder with the WASM files>` uses the builders the way a wallet
flow would, including the refusals (one run is in
[`docs/evidence`](../../docs/evidence/testnet-sdk-writes-2026-10-07.md)).

The same plumbing runs the reference consumer end to end (`npm run testnet:gated-claim -w @sorogate/sdk -- --wasm-dir <the folder
with the three WASM files> [--policy-contract <C...>]`): a claim, a refused double claim, the owner changing the rule, a
pinned consumer refusing it, and a claim signed by someone else. With `--policy-contract` it uses a policy contract that is
already deployed (the public one in [`docs/DEPLOYMENT.md`](../../docs/DEPLOYMENT.md)) after checking its code against the
local build. One run is recorded in [`docs/evidence`](../../docs/evidence/testnet-gated-claim-2026-10-07.md).

`npm run testnet:sep50 -w @sorogate/sdk -- --wasm-dir <the folder with the WASM files> --nft-wasm <a SEP-50 collection's WASM> --nft-source <where it came from> --policy-contract <C...>`
checks the `NftBalance` condition against a real collection (OpenZeppelin's example, built from its own repository), comparing the
contract with the model and running the consumer. One run is recorded in
[`docs/evidence`](../../docs/evidence/testnet-sep50-2026-10-07.md).

`npm run testnet:costs -w @sorogate/sdk -- --wasm-dir <the folder with the WASM files> --ft-wasm <a fungible token's WASM> --nft-wasm <an NFT collection's WASM> --sources <where they came from> --report costs.json`
measures what `evaluate`, `create` and a consumer's `claim` cost on Testnet over real contracts; the results and how to read them
are in [`docs/COSTS.md`](../../docs/COSTS.md).

## Errors

| Error | Meaning |
| --- | --- |
| `ContractCallError` | The contract returned one of its own errors; `errorName` is for example `PolicyNotFound`. |
| `SimulationError` | The simulation failed for any other reason. |
| `LedgerMovedError` | `fetchSnapshot` could not get a consistent read in the allowed number of attempts. |
| `DecodeError` | The contract returned something this package does not recognise. |

## Develop

```bash
npm ci
npm test            # unit tests, the shared vectors, and the codec against recorded contract output
npm run lint && npm run typecheck && npm run build
```

`test/fixtures/testnet-recordings.json` holds real return values of the deployed contract, recorded from Testnet,
with their provenance. They are why the codec is tested against the contract's own bytes and not only against
shapes written by hand.
