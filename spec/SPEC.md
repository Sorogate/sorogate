# Access policy specification (draft 0.1)

This document is the source of truth for what an access policy means. The Soroban contract in
`contracts/access-policy` implements it. The TypeScript model in `packages/sdk` must give the same answers.
Where the code and this document disagree, that is a bug in one of them, and the shared test vectors
(`spec/vectors/`) decide which.

Status: draft. Testnet only. Not audited.

## 1. What a policy is

An **access policy** is a stored list of conditions about a **subject** (an `Address`, either a classic
account `G...` or a contract account `C...`). A policy is **allowed** for a subject when every condition holds.

Evaluating a policy answers one question: *does this subject satisfy the policy at this ledger?*
It does not prove that the caller is the subject. See section 8.

(An access policy is not an OpenZeppelin smart-account `Policy`. That is a trait whose `enforce()` panics when
conditions fail and may change state. An access policy returns a decision and changes nothing.)

## 2. Data model

| Type | Fields |
| --- | --- |
| `Policy` | `owner: Address`, `version: u32`, `active: bool`, `conditions: Vec<Condition>` |
| `Condition` | one of `TokenBalance(TokenBalanceCond)`, `NftBalance(NftBalanceCond)`, `TimeWindow(TimeWindowCond)` |
| `TokenBalanceCond` | `token: Address`, `min: i128` |
| `NftBalanceCond` | `collection: Address`, `min: u32` |
| `TimeWindowCond` | `not_before: Option<u64>`, `not_after: Option<u64>` (unix seconds) |
| `Decision` | `allowed: bool`, `version: u32`, `failed_index: Option<u32>`, `reason: DenyReason` |

`min` is in the token's **base units** (for a Stellar asset with 7 decimals, 1 whole unit is 10,000,000).

## 3. Validity (checked when a policy is created or updated)

A list of conditions is valid when all of these hold; otherwise the write fails with the listed error and
nothing changes.

| Rule | Error (code) |
| --- | --- |
| At least one condition | `NoConditions` (2) |
| At most `MAX_CONDITIONS` = 8 conditions | `TooManyConditions` (3) |
| `TokenBalance.min > 0` and `NftBalance.min > 0` | `InvalidMinimum` (4) |
| `TimeWindow` has at least one bound, and if both, `not_before < not_after` | `InvalidTimeWindow` (5) |
| `token` / `collection` is a deployed contract (WASM or Stellar asset contract), not an account or an empty address | `NotAContract` (6) |

Rules are checked in the order of the conditions; within one condition, in the order of the table above.
An unknown policy id is `PolicyNotFound` (1).

Why the contract check: calling an account address as if it were a contract aborts the whole transaction
instead of failing softly (measured on Testnet, 2026-10-06, see
[`docs/evidence/spike-2026-10-06.md`](../docs/evidence/spike-2026-10-06.md)), so such an address must never be stored.

## 4. Evaluation

`evaluate(id, subject)` is read-only, needs no authorization, and returns a `Decision`.

1. If no policy has this id: error `PolicyNotFound`. This is not a denial.
2. `version` in the decision is the stored version of the policy.
3. If the policy is inactive: `allowed = false`, `reason = Inactive`, `failed_index = None`.
4. Otherwise let `now` be the ledger timestamp. Evaluate the conditions **in order** and **stop at the first
   one that does not hold**; conditions after it are not evaluated.
   - If one fails: `allowed = false`, `failed_index = Some(its index)`, `reason` as below.
   - If none fails: `allowed = true`, `failed_index = None`, `reason = None`.

### 4.1 Conditions

- **TokenBalance.** Call `balance(subject)` on `token` and read the result as `i128`.
  Holds when `balance >= min`. If the balance was read and is below `min`: `BelowMinimum`.
- **NftBalance.** Call `balance(subject)` on `collection` and read the result as `u32`.
  Holds when `balance >= min`; below: `BelowMinimum`.
- **BalanceUnavailable.** In both cases above, if the call raises an error, panics, does not exist, or
  returns a value that is not of the expected type (for example a `u64` or `i128` where `u32` is expected),
  the condition does not hold and the reason is `BalanceUnavailable`. This is **fail closed**.
  - For classic assets this is the normal answer for "does not hold this asset": a Stellar asset contract
    raises an error ("trustline entry is missing") instead of returning 0 (measured on Testnet).
    Readers of a decision must not treat `BalanceUnavailable` as an outage.
- **TimeWindow.** Holds when `not_before <= now` (if set) and `now < not_after` (if set). Earlier than
  `not_before`: `BeforeWindow`. At or after `not_after`: `AfterWindow`. When both fail to hold at once
  (impossible for a valid window) `BeforeWindow` is reported first.

### 4.2 Time

`now` is `ledger().timestamp()`, the close time of the ledger in which the call runs, in unix seconds.
A simulation runs against the **latest closed ledger**; the transaction it predicts is applied in a **later**
ledger (at least one, about 5 seconds each). A result obtained by simulation is therefore advisory near a
window edge; the result obtained when the transaction executes is authoritative.

### 4.3 What a balance read covers

Only the single function `balance(Address)` of SEP-41 (`-> i128`) and SEP-50 (`-> u32`). Both standards are
Drafts. Allowances, events, metadata, `owner_of`, `token_uri` and transfers are not used. If either standard
changes `balance`, this specification gets a new version.

Informative: SEP-50 (Draft 0.1.0) describes the return type only as "an unsigned integer", usually the same type as
the token id; it does not say `u32`. The one real collection tried, OpenZeppelin's `nft-sequential-minting` example at
v0.7.2, returns a `u32`
([recorded run](../docs/evidence/testnet-sep50-2026-10-07.md)). A collection that returns a wider type is read as
unavailable, and the condition fails closed.

## 5. Encodings

`DenyReason` is an integer.

| Code | Name |
| --- | --- |
| 0 | `None` (allowed) |
| 1 | `Inactive` |
| 2 | `BelowMinimum` |
| 3 | `BalanceUnavailable` |
| 4 | `BeforeWindow` |
| 5 | `AfterWindow` |

Invariant: `reason == None` exactly when `allowed == true`. `failed_index` is `None` when `allowed == true`
and when `reason == Inactive`; otherwise it is `Some(i)` with `i < conditions.len()`.

## 6. Lifecycle and ownership

| Function | Who | Effect |
| --- | --- | --- |
| `create(owner, conditions) -> id` | `owner` authorizes | Validates. Ids are sequential from 1. `version = 1`, `active = true`. |
| `update(id, conditions)` | stored owner | Validates, replaces the conditions, `version += 1`. |
| `set_active(id, active)` | stored owner | Changes `active`. Does **not** change `version`. |
| `get(id) -> Policy` | anyone | Read-only. |
| `evaluate(id, subject) -> Decision` | anyone | Read-only. |
| `bump(id)` | anyone | Extends the lifetime of the policy and of the contract (its instance and its code). |

`set_active` with the value the policy already has succeeds. It stores the policy as it was, extends the lifetimes as any
write does, does not change `version`, and still publishes `active_changed`, so an `active_changed` event does not by itself
mean that `active` changed.

There is no ownership transfer, no deletion and no administrator. An abandoned policy is deactivated.
`version` lets a consumer notice that the rules behind an id changed; a consumer that wants fixed rules
compares `Decision.version` with the version it reviewed and refuses otherwise.

Events (topics `["access_policy", name]`, indexed topic `id`): `created` (data `owner`, `version`),
`updated` (data `version`), `active_changed` (data `active`).

## 7. Lifetime

Policies are stored in persistent storage and are never evicted for good: an expired entry is archived and can
be restored. Writes and `bump` top the entry up to 90 days (writes top up when it has less than 30 days left), and with it the contract's
instance and code, which live only about 7 days after deployment until one of them happens
([recorded run](../docs/evidence/testnet-ttl-2026-10-07.md)).
A transaction that touches an archived policy must include it in its restore list; the RPC simulation adds that
automatically. Restoring costs a fee but does not change any decision.

## 8. What this does not do

- **It does not authenticate the subject.** `evaluate(id, X)` answers for any address `X` that anyone passes.
  A contract that gives `X` something must also call `X.require_auth()`, or anyone can claim on behalf of any
  qualifying address.
- **A balance is a snapshot, not an identity.** The same tokens can satisfy a policy for one address, move, and
  satisfy it for another. A flash loan can satisfy it for one transaction.
- **The policy owner chooses the tokens.** A token can burn its whole budget; that aborts the calling
  transaction and cannot be caught. A consumer should only use policies whose owner it trusts.
- **It is not secrecy.** Gating content in a web page does not hide the content; only a server that checks and
  holds the content can.

## 9. Not in this version

OR / NOT, arbitrary external calls, credentials, specific NFT ids, ownership transfer, batch evaluation, a
mainnet deployment. Credentials are an adapter interface in the SDK only.
