# Sorogate

Reusable **access policies** for Stellar, stored in a Soroban contract and evaluated the same way from
other contracts and from TypeScript.

> **Status: early development. Testnet only. Not audited. Not production.**
> There is one public **development deployment on Stellar Testnet**, described in [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md). It is
> not a production deployment, and Testnet is reset from time to time. The other Testnet runs in `docs/evidence` used
> throwaway deployments.

An access policy is a short list of conditions about an address: *holds at least N of this token*, *holds at
least N of this collection*, *the ledger time is inside this window*. All conditions must hold. A contract (or an
app) asks `evaluate(policy_id, address)` and gets back a decision with the reason when it is a denial. Because the
rules live in one place, an owner can change them without redeploying every contract that depends on them.

## Try it, and see what it looks like

- **In a browser, with nothing to install:** the [playground](https://sorogate.github.io/sorogate/) works a policy out and sets the
  result beside the answer the contract's tests require.
- **From a contract.** Three steps, in this order. The [example consumer](https://github.com/Sorogate/example-consumer) runs this
  against the public deployment, with tests:

  ```rust
  pub fn enter(env: Env, subject: Address) -> Result<(), Error> {
      subject.require_auth();                        // 1. Prove who is asking. `evaluate` does not.
      let decision = match PolicyClient::new(&env, &policy_contract).try_evaluate(&policy_id, &subject) {
          Ok(Ok(decision)) => decision,              // 2. Ask the policy,
          _ => return Err(Error::PolicyUnavailable), //    and treat a failure to get an answer as a no.
      };
      if !decision.allowed { return Err(/* say why, from decision.reason */); }   // 3. Enforce the answer.
      /* record it, then act */
  }
  ```

- **From TypeScript:** see [`packages/sdk`](packages/sdk/README.md#using-it-today). The SDK is not published yet, and that page says how
  to use it from this repository.

## What it is not

- It does **not** prove who is calling. `evaluate` answers for any address anyone passes in, so a contract that
  acts on the answer must also call `address.require_auth()`. See [spec/SPEC.md](spec/SPEC.md) section 8.
- A balance is a snapshot, not an identity: the same tokens can pass the check for different addresses in turn.
- It is not secrecy. Hiding content behind a client-side check does not hide it.
- It is **not** an OpenZeppelin smart-account `Policy` (their `enforce()` panics and may change state; ours returns
  a decision and changes nothing).
- It does not issue or verify credentials. Credentials are an adapter interface planned for the SDK.
- It is not affiliated with or endorsed by the Stellar Development Foundation. "Stellar" and "Soroban" are their
  trademarks.

## What exists

- **The contract**, [`contracts/access-policy`](contracts/access-policy): `create`, `update`, `set_active`, `get`, `evaluate`, `bump`, with 27 unit tests,
  including a Stellar asset contract and deliberately broken tokens.
- **The model and SDK**, [`packages/sdk`](packages/sdk): a TypeScript model of the rules, a read-only client for a deployed contract, and
  builders for unsigned transactions. Not published. The contract, not the model, is the authoritative answer.
- **The playground**, [`packages/site`](packages/site): a static page, live at <https://sorogate.github.io/sorogate/>. It makes no network calls and holds no keys.
- **Shared test vectors**, [`spec/vectors`](spec/vectors): 70 shared test cases (45 decisions, 25 validity checks) that the contract and the
  TypeScript model must both pass, plus thousands of random cases on every push (CI runs four seeds of 1,000 per file).
- **Two example consumers.** [`contracts/gated-claim`](contracts/gated-claim) pays a fixed amount once to each qualifying address and can pin the
  policy version; it is a demonstration with no withdrawal and no administrator, so fund it only with a test asset.
  [`Sorogate/example-consumer`](https://github.com/Sorogate/example-consumer) is a separate, tiny repository with a "gate" that uses only the published
  interface: Sorogate is the reusable primitive, and this is an independent integration example, not an official or production
  one. It does not import this repository's code; it declares the interface by hand and its tests run against the deployed
  contract's code, pinned by hash. So far nobody outside this project has used it. [`contracts/mock-token`](contracts/mock-token) is a test fixture
  token that anyone can set any balance on.
- **A public Testnet deployment**, [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md): address, code hash, date, who controls it (nobody), how to check
  that it is the code in this repository, and how to keep it alive.
- **Recorded Testnet runs**, [`docs/evidence`](docs/evidence): the deployed contract and the model agreed on 120 comparisons; the reference
  consumer was walked through its scenarios on the public deployment (26 steps, all as expected, including refusals that were applied and failed
  as real transactions); and the `NftBalance` condition was checked against a real OpenZeppelin SEP-50 collection
  (29 steps, contract and model agreeing on every comparison). Those collections and tokens were ones we deployed, not ones somebody else operates.
- **Costs**, [`docs/COSTS.md`](docs/COSTS.md): what `evaluate`, `create` and a consumer's `claim` cost on Testnet over real tokens, and what
  the numbers say about the limit of 8 conditions (for the tokens measured, they give no reason to change it).
- **The documents.** [`docs/README.md`](docs/README.md) indexes them by question; [`docs/EVIDENCE.md`](docs/EVIDENCE.md) says what the words used for
  evidence (Unit-tested, Recorded, Live, Simulated, Example) mean. The rules are in [`spec/SPEC.md`](spec/SPEC.md), what is and is not protected is
  in [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md), and how to use a policy from your own contract is in [`docs/INTEGRATING.md`](docs/INTEGRATING.md).

## What is planned

Nothing is promised. Nothing here is called useful until a contract nobody here wrote depends on it.

## Build and test

You need Rust (the version in `rust-toolchain.toml`, with the `wasm32v1-none` target), the
[Stellar CLI](https://github.com/stellar/stellar-cli) 25.2 or newer, and Node 22.12 or newer (what the SDK's dependencies and the web page's build declare; the web page's tests use jsdom, which
declares 22.22.2 or newer and prints a warning on older versions). Contracts built with soroban-sdk 28 must be built with `stellar contract build`; a plain `cargo build` is refused.

```bash
cargo test --workspace          # contract unit tests and the vectors
stellar contract build          # release WASM
npm ci && npm test              # the TypeScript model and the vectors
```

To compare the contract with the TypeScript model on random policies, see
[spec/vectors/README.md](spec/vectors/README.md).

On Windows, run the Rust tests inside WSL: native Windows linking of soroban-sdk's test utilities fails.
`scripts/wsl-test.sh` and `scripts/wsl-check.sh` (format, clippy, tests, WASM build) keep build output on the
Linux filesystem, and `scripts/wsl-js.sh` does the same for the TypeScript checks.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Scoped candidates for new contributors are in
[ISSUES_BACKLOG.md](ISSUES_BACKLOG.md). A security problem goes through [SECURITY.md](SECURITY.md), not a public issue.

## Licence

Apache-2.0, see [LICENSE](LICENSE).
