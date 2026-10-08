# Contributing

Thank you for looking. Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) first for how the pieces fit, and
[`spec/SPEC.md`](spec/SPEC.md), which is the source of truth for what a policy means.

## What you need

- **Rust**, through [rustup](https://rustup.rs). The repository pins the version in `rust-toolchain.toml`, and rustup
  installs it (with the `wasm32v1-none` target) the first time you run `cargo` here.
- The **[Stellar CLI](https://github.com/stellar/stellar-cli)**, 25.2 or newer. CI uses 28.1.0; the runs recorded in `docs/evidence` used 27.1.0.
  Contracts built with soroban-sdk 28 must be built with `stellar contract build`; a plain `cargo build` is refused.
- **Node** 22.22.2 or newer for development. The SDK itself supports Node 22.12.0 or newer.
- **No Docker and no wallet.** Everything in the daily loop runs without them.

The first Rust build compiles every dependency and takes several minutes (about eight in total for the three steps
below, from an empty cache, on a four-thread machine). Later runs are much faster.

On **Windows**, run the Rust commands inside WSL. Linking soroban-sdk's test utilities natively on Windows fails.
Work from the Linux filesystem (for example `~/sorogate`) if you can: tests run far faster there than under `/mnt/c`.

```bash
git clone https://github.com/Sorogate/sorogate.git
cd sorogate
```

## The daily loop

The contracts:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
stellar contract build
```

The TypeScript package:

```bash
npm ci
npm run lint
npm run typecheck
npm test
npm run build
```

CI runs exactly these, plus a random comparison of the contract with the TypeScript model (see below).

## Changing behaviour

What a policy means is written down in [`spec/SPEC.md`](spec/SPEC.md) and pinned by the cases in
[`spec/vectors`](spec/vectors). Two implementations must give the same answer for every case: the contract
(`contracts/access-policy`) and the TypeScript model (`packages/sdk`). So a change to behaviour is one pull request that:

1. updates `spec/SPEC.md`,
2. updates the vectors,
3. updates **both** implementations,
4. and passes both test suites.

A pull request that changes only one of them will fail the vectors, which is the point.

To compare the two on thousands of random policies on your own machine:

```bash
npm run generate:random -w @sorogate/sdk -- --seed 1 --count 1000 --out ../../spec/vectors/generated
SOROGATE_RANDOM_VECTORS="$PWD/spec/vectors/generated" cargo test -p access-policy --test vectors random
```

(`--out` is relative to `packages/sdk`; the second command needs an absolute path.) If this fails, it prints the cases
that disagree, and the seed lets anyone reproduce it.

## What needs tests

- **A test that fails when the change is undone.** Do not stop at "the tests pass". Break the code on purpose (flip a
  `>=`, delete an authorization check) and confirm a test notices. The reviews of this project were done that way, and
  every bug that survived was either closed with a new test or recorded as having no observable effect.
- **The contract**: unit tests in `contracts/*/src/test.rs`, using the real contracts and the `mock-token` fixture.
  Test the refusals as carefully as the pass.
- **The TypeScript model and client**: `packages/sdk/test`. The client is tested against a stand-in RPC server, and the
  codec against real contract output recorded in `packages/sdk/test/fixtures`.
- **Anything about the real network** is checked by the manual scripts in `packages/sdk/scripts`, not by CI. If you add
  or change one, record a run in [`docs/evidence`](docs/evidence).

## Evidence and claims

A statement about behaviour in the documentation is accepted only with a test or a recorded run behind it. When you
record a run, label it (`Recorded`), say what it does **not** show as plainly as what it does, and do not round up.
The existing notes in `docs/evidence` are the model. If a run finds something you did not expect, that goes in the note.

## Style

- Rust: default `rustfmt`, and `clippy` with warnings as errors. Contracts are `no_std`, with no `unsafe`.
- TypeScript: strict mode, ES modules, relative imports end in `.js` even though the files are `.ts` (the
  `NodeNext` convention), and ESLint enforces the rest.
- Comments say why, not what.

## Scope

Some things are out of scope on purpose, and a pull request for them needs a discussion in an issue first:
arbitrary external calls inside a policy, credential issuance or verification, an administrator or upgrade key with
power over other people's policies, holding user funds, secrecy claims, and any Mainnet deployment. The full list is in
[`spec/SPEC.md`](spec/SPEC.md) section 9 and [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md).

`contracts/mock-token` is a test fixture. Anyone can set any balance on it. Never deploy it anywhere that matters.

## Finding something to work on

[`ISSUES_BACKLOG.md`](ISSUES_BACKLOG.md) lists scoped candidates, each with its current state, what to build and how to
verify it, rated **Trivial**, **Medium** or **High** by scope and complexity:

- **Trivial**: a small bug fix, better error text, a test for something untested.
- **Medium**: a standard new feature or an involved fix: a new check in CI, a new recorded run.
- **High**: a complex feature, a refactor, or research that may change a design decision.

Open issues are on the [issues page](https://github.com/Sorogate/sorogate/issues); the ones marked `good first issue` are
the smallest.

## Picking up an issue

Comment on the issue to say you would like it, and wait for the maintainer to assign it to you before you start. A comment
alone does not reserve it. If an assigned issue has had no activity for 7 days, the maintainer may ask whether you are still
working on it, and may unassign it after 7 more days without a reply. A pull request for an issue that is assigned to someone
else is looked at after theirs.

## Your first pull request

The first time you open a pull request, GitHub holds its CI run until a maintainer approves it, so the checks show nothing for
a while. That is a GitHub setting, not broken CI. The maintainer approves the run when they review. Run the daily loop above
locally in the meantime.

## AI-assisted contributions

AI-assisted contributions are welcome, as is this project's own use of AI assistance. You are responsible for what you submit:
you have run it, you understand it, and every claim in the description is true. In this project that includes any claim in the
documentation: it must rest on a test or a recorded run (see "Evidence and claims"). Pull requests are reviewed the same way
whoever or whatever wrote them.

## How maintainers work here

- There is currently one maintainer. Response times are best effort, with no guaranteed turnaround.
- A bug is reproduced before a fix is accepted. A feature is discussed in its issue before a pull request.
- CI must pass before merge. Every GitHub Action in `.github/workflows` is pinned to a commit hash, with its tag in a comment;
  Dependabot updates them. Pin a new one the same way, and check that it still works pinned: the `stellar/stellar-cli` action, for one, takes the CLI version from the ref it is called with, so it needs its `version:` input set once the ref is a hash. A separate `Audit` workflow checks the locked dependencies against published
  advisories (weekly, on pushes to `main`, and on a pull request that changes a dependency file); a finding there is a signal to
  look at, not a merge block, and the known exceptions are listed in `docs/THREAT_MODEL.md`.
- Changes to what a policy means (the specification, the vectors, the error and reason codes) need maintainer approval.

## Reporting a security problem

Not in a public issue. See [`SECURITY.md`](SECURITY.md).
