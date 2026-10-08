extern crate std;

use soroban_sdk::{
    testutils::{
        storage::Persistent as _, Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke,
    },
    token::StellarAssetClient,
    Address, Env, Event as _, IntoVal, Vec,
};

use crate::{
    AccessPolicy, AccessPolicyClient, ActiveChanged, Condition, Created, DataKey, Decision,
    DenyReason, Error, NftBalanceCond, TimeWindowCond, TokenBalanceCond, Updated, DAY_IN_LEDGERS,
    MAX_CONDITIONS, TTL_EXTEND_TO, TTL_THRESHOLD,
};

mod mocks {
    use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env};

    fn count_call(env: &Env) {
        let n: u32 = env
            .storage()
            .instance()
            .get(&symbol_short!("calls"))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&symbol_short!("calls"), &(n + 1));
    }

    /// A SEP-41 shaped token: `balance(Address) -> i128`. Counts how often it was read.
    #[contract]
    pub struct MockToken;

    #[contractimpl]
    impl MockToken {
        pub fn set_balance(env: Env, who: Address, amount: i128) {
            env.storage().persistent().set(&who, &amount);
        }
        pub fn balance(env: Env, id: Address) -> i128 {
            count_call(&env);
            env.storage().persistent().get(&id).unwrap_or(0)
        }
        pub fn calls(env: Env) -> u32 {
            env.storage()
                .instance()
                .get(&symbol_short!("calls"))
                .unwrap_or(0)
        }
    }

    /// A SEP-50 shaped collection: `balance(Address) -> u32` (what OpenZeppelin implements).
    #[contract]
    pub struct MockNft;

    #[contractimpl]
    impl MockNft {
        pub fn set_balance(env: Env, who: Address, amount: u32) {
            env.storage().persistent().set(&who, &amount);
        }
        pub fn balance(env: Env, id: Address) -> u32 {
            env.storage().persistent().get(&id).unwrap_or(0)
        }
    }

    /// Returns a `u64` balance: a legal reading of SEP-50's "unsigned integer", but not `u32`.
    #[contract]
    pub struct WideBalance;

    #[contractimpl]
    impl WideBalance {
        pub fn balance(_env: Env, _id: Address) -> u64 {
            5
        }
    }

    #[contract]
    pub struct PanicToken;

    #[contractimpl]
    impl PanicToken {
        pub fn balance(_env: Env, _id: Address) -> i128 {
            panic!("boom")
        }
    }
}

use mocks::{MockNft, MockNftClient, MockToken, MockTokenClient, PanicToken, WideBalance};

// ---------------------------------------------------------------- helpers

fn new_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

fn client(env: &Env) -> AccessPolicyClient<'_> {
    AccessPolicyClient::new(env, &env.register(AccessPolicy, ()))
}

fn token_with(env: &Env, holder: &Address, amount: i128) -> Address {
    let token = env.register(MockToken, ());
    MockTokenClient::new(env, &token).set_balance(holder, &amount);
    token
}

fn nft_with(env: &Env, holder: &Address, amount: u32) -> Address {
    let nft = env.register(MockNft, ());
    MockNftClient::new(env, &nft).set_balance(holder, &amount);
    nft
}

fn tb(token: &Address, min: i128) -> Condition {
    Condition::TokenBalance(TokenBalanceCond {
        token: token.clone(),
        min,
    })
}

fn nb(collection: &Address, min: u32) -> Condition {
    Condition::NftBalance(NftBalanceCond {
        collection: collection.clone(),
        min,
    })
}

fn tw(not_before: Option<u64>, not_after: Option<u64>) -> Condition {
    Condition::TimeWindow(TimeWindowCond {
        not_before,
        not_after,
    })
}

fn list(env: &Env, items: &[Condition]) -> Vec<Condition> {
    Vec::from_slice(env, items)
}

fn allowed(version: u32) -> Decision {
    Decision {
        allowed: true,
        version,
        failed_index: None,
        reason: DenyReason::None,
    }
}

fn denied(version: u32, failed_index: Option<u32>, reason: DenyReason) -> Decision {
    Decision {
        allowed: false,
        version,
        failed_index,
        reason,
    }
}

fn set_time(env: &Env, timestamp: u64) {
    env.ledger().with_mut(|l| l.timestamp = timestamp);
}

fn policy_ttl(env: &Env, c: &AccessPolicyClient, id: u64) -> u32 {
    env.as_contract(&c.address, || {
        env.storage().persistent().get_ttl(&DataKey::Policy(id))
    })
}

// ---------------------------------------------------------------- lifecycle

#[test]
fn create_assigns_sequential_ids_and_stores_the_policy() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = token_with(&env, &holder, 100);

    let conditions = list(&env, &[tb(&token, 100), tw(Some(10), None)]);
    assert_eq!(c.create(&owner, &conditions), 1);
    assert_eq!(c.create(&owner, &list(&env, &[tb(&token, 1)])), 2);

    let stored = c.get(&1);
    assert_eq!(stored.owner, owner);
    assert_eq!(stored.version, 1);
    assert!(stored.active);
    assert_eq!(stored.conditions, conditions);
    assert_eq!(c.get(&2).conditions.len(), 1);
}

#[test]
fn unknown_ids_are_errors_not_denials() {
    let env = new_env();
    let c = client(&env);
    let who = Address::generate(&env);
    let conditions = list(&env, &[tw(Some(1), None)]);

    assert_eq!(c.try_get(&7), Err(Ok(Error::PolicyNotFound)));
    assert_eq!(c.try_evaluate(&7, &who), Err(Ok(Error::PolicyNotFound)));
    assert_eq!(
        c.try_update(&7, &conditions),
        Err(Ok(Error::PolicyNotFound))
    );
    assert_eq!(c.try_set_active(&7, &false), Err(Ok(Error::PolicyNotFound)));
    assert_eq!(c.try_bump(&7), Err(Ok(Error::PolicyNotFound)));
}

#[test]
fn create_requires_the_owners_authorization() {
    let env = Env::default(); // no mocked auths
    let c = client(&env);
    let owner = Address::generate(&env);
    assert!(c
        .try_create(&owner, &list(&env, &[tw(Some(1), None)]))
        .is_err());
}

#[test]
fn only_the_owner_can_update_or_deactivate() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let intruder = Address::generate(&env);
    let id = c.create(&owner, &list(&env, &[tw(Some(1), None)]));
    let new_conditions = list(&env, &[tw(Some(2), None)]);

    env.mock_auths(&[MockAuth {
        address: &intruder,
        invoke: &MockAuthInvoke {
            contract: &c.address,
            fn_name: "update",
            args: (id, new_conditions.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(c.try_update(&id, &new_conditions).is_err());

    env.mock_auths(&[MockAuth {
        address: &intruder,
        invoke: &MockAuthInvoke {
            contract: &c.address,
            fn_name: "set_active",
            args: (id, false).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(c.try_set_active(&id, &false).is_err());

    let stored = c.get(&id);
    assert_eq!(stored.version, 1);
    assert!(stored.active);
}

#[test]
fn update_replaces_conditions_and_increases_the_version() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = token_with(&env, &holder, 150);

    let id = c.create(&owner, &list(&env, &[tb(&token, 100)]));
    assert_eq!(c.evaluate(&id, &holder), allowed(1));

    // The owner raises the bar. The consumer is not touched and sees the new rule at once.
    c.update(&id, &list(&env, &[tb(&token, 500)]));
    assert_eq!(c.get(&id).version, 2);
    assert_eq!(
        c.evaluate(&id, &holder),
        denied(2, Some(0), DenyReason::BelowMinimum)
    );
}

#[test]
fn deactivating_denies_everyone_and_does_not_change_the_version() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = token_with(&env, &holder, 100);
    let id = c.create(&owner, &list(&env, &[tb(&token, 1)]));

    c.set_active(&id, &false);
    assert_eq!(
        c.evaluate(&id, &holder),
        denied(1, None, DenyReason::Inactive)
    );

    // Updating an inactive policy keeps it inactive.
    c.update(&id, &list(&env, &[tb(&token, 2)]));
    assert_eq!(
        c.evaluate(&id, &holder),
        denied(2, None, DenyReason::Inactive)
    );

    c.set_active(&id, &true);
    assert_eq!(c.evaluate(&id, &holder), allowed(2));
}

// `set_active` with the value a policy already has. The spec says what it changes (`active`) and what it leaves alone
// (`version`), not what a call that changes nothing does. These two tests document what the contract does today: the call
// succeeds, the version and the decision are untouched, and `active_changed` is published again although nothing changed. They
// describe the behaviour, they do not endorse it. If a no-op should publish nothing, that is a change to the contract and to the
// spec, and belongs in its own issue.

#[test]
fn setting_active_on_an_active_policy_succeeds_and_still_publishes_active_changed() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = token_with(&env, &holder, 100);
    let id = c.create(&owner, &list(&env, &[tb(&token, 1)]));

    c.set_active(&id, &true);

    assert_eq!(
        env.events().all(),
        std::vec![ActiveChanged { id, active: true }.to_xdr(&env, &c.address)]
    );
    assert_eq!(c.get(&id).version, 1);
    assert!(c.get(&id).active);
    assert_eq!(c.evaluate(&id, &holder), allowed(1));
}

#[test]
fn setting_active_to_false_on_an_inactive_policy_succeeds_and_still_publishes_active_changed() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = token_with(&env, &holder, 100);
    let id = c.create(&owner, &list(&env, &[tb(&token, 1)]));
    c.set_active(&id, &false);

    c.set_active(&id, &false);

    assert_eq!(
        env.events().all(),
        std::vec![ActiveChanged { id, active: false }.to_xdr(&env, &c.address)]
    );
    assert_eq!(c.get(&id).version, 1);
    assert!(!c.get(&id).active);
    assert_eq!(
        c.evaluate(&id, &holder),
        denied(1, None, DenyReason::Inactive)
    );
}

#[test]
fn evaluate_needs_no_authorization() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = token_with(&env, &holder, 10);
    let id = c.create(&owner, &list(&env, &[tb(&token, 10)]));

    env.mock_auths(&[]); // nobody has authorized anything
    assert_eq!(c.evaluate(&id, &holder), allowed(1));
    assert_eq!(c.get(&id).owner, owner);
}

#[test]
fn lifecycle_calls_publish_events() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let id = c.create(&owner, &list(&env, &[tw(Some(1), None)]));
    assert_eq!(
        env.events().all(),
        std::vec![Created {
            id,
            owner: owner.clone(),
            version: 1
        }
        .to_xdr(&env, &c.address)]
    );

    c.update(&id, &list(&env, &[tw(Some(2), None)]));
    assert_eq!(
        env.events().all(),
        std::vec![Updated { id, version: 2 }.to_xdr(&env, &c.address)]
    );

    c.set_active(&id, &false);
    assert_eq!(
        env.events().all(),
        std::vec![ActiveChanged { id, active: false }.to_xdr(&env, &c.address)]
    );
}

// ---------------------------------------------------------------- validation

#[test]
fn rejects_empty_and_oversized_policies() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);

    assert_eq!(
        c.try_create(&owner, &list(&env, &[])),
        Err(Ok(Error::NoConditions))
    );

    let one = tw(Some(1), None);
    let at_limit: std::vec::Vec<Condition> = (0..MAX_CONDITIONS).map(|_| one.clone()).collect();
    let over_limit: std::vec::Vec<Condition> = (0..=MAX_CONDITIONS).map(|_| one.clone()).collect();
    assert_eq!(c.try_create(&owner, &list(&env, &at_limit)), Ok(Ok(1)));
    assert_eq!(
        c.try_create(&owner, &list(&env, &over_limit)),
        Err(Ok(Error::TooManyConditions))
    );
}

#[test]
fn rejects_non_positive_minimums() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let token = token_with(&env, &holder, 1);
    let nft = nft_with(&env, &holder, 1);

    assert_eq!(
        c.try_create(&owner, &list(&env, &[tb(&token, 0)])),
        Err(Ok(Error::InvalidMinimum))
    );
    assert_eq!(
        c.try_create(&owner, &list(&env, &[tb(&token, -5)])),
        Err(Ok(Error::InvalidMinimum))
    );
    assert_eq!(
        c.try_create(&owner, &list(&env, &[nb(&nft, 0)])),
        Err(Ok(Error::InvalidMinimum))
    );
}

#[test]
fn rejects_malformed_time_windows() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);

    for bad in [
        tw(None, None),
        tw(Some(100), Some(100)),
        tw(Some(200), Some(100)),
    ] {
        assert_eq!(
            c.try_create(&owner, &list(&env, &[bad])),
            Err(Ok(Error::InvalidTimeWindow))
        );
    }
    for good in [
        tw(Some(100), None),
        tw(None, Some(100)),
        tw(Some(100), Some(101)),
    ] {
        assert!(c.try_create(&owner, &list(&env, &[good])).is_ok());
    }
}

#[test]
fn rejects_addresses_that_are_not_deployed_contracts() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    // Calling an account address as a contract aborts the whole transaction (measured on Testnet), so it
    // must be refused when the policy is written.
    let account = Address::from_str(
        &env,
        "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L",
    );
    let nothing_deployed = Address::generate(&env);

    assert_eq!(
        c.try_create(&owner, &list(&env, &[tb(&account, 1)])),
        Err(Ok(Error::NotAContract))
    );
    assert_eq!(
        c.try_create(&owner, &list(&env, &[tb(&nothing_deployed, 1)])),
        Err(Ok(Error::NotAContract))
    );
    assert_eq!(
        c.try_create(&owner, &list(&env, &[nb(&account, 1)])),
        Err(Ok(Error::NotAContract))
    );
}

#[test]
fn update_applies_the_same_validation_and_leaves_the_policy_untouched_on_error() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let id = c.create(&owner, &list(&env, &[tw(Some(1), None)]));

    assert_eq!(
        c.try_update(&id, &list(&env, &[])),
        Err(Ok(Error::NoConditions))
    );
    assert_eq!(
        c.try_update(&id, &list(&env, &[tw(None, None)])),
        Err(Ok(Error::InvalidTimeWindow))
    );
    let stored = c.get(&id);
    assert_eq!(stored.version, 1);
    assert_eq!(stored.conditions, list(&env, &[tw(Some(1), None)]));
}

// ---------------------------------------------------------------- token balance

#[test]
fn token_balance_boundaries() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let (below, exact, above, never_set) = (
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    );
    let token = env.register(MockToken, ());
    let t = MockTokenClient::new(&env, &token);
    t.set_balance(&below, &99);
    t.set_balance(&exact, &100);
    t.set_balance(&above, &101);

    let id = c.create(&owner, &list(&env, &[tb(&token, 100)]));
    assert_eq!(
        c.evaluate(&id, &below),
        denied(1, Some(0), DenyReason::BelowMinimum)
    );
    assert_eq!(c.evaluate(&id, &exact), allowed(1));
    assert_eq!(c.evaluate(&id, &above), allowed(1));
    assert_eq!(
        c.evaluate(&id, &never_set),
        denied(1, Some(0), DenyReason::BelowMinimum)
    );
}

#[test]
fn token_balance_at_the_largest_amount() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let whale = Address::generate(&env);
    let token = token_with(&env, &whale, i128::MAX);

    let id = c.create(&owner, &list(&env, &[tb(&token, i128::MAX)]));
    assert_eq!(c.evaluate(&id, &whale), allowed(1));
}

#[test]
fn a_token_that_panics_is_balance_unavailable() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);
    let token = env.register(PanicToken, ());

    let id = c.create(&owner, &list(&env, &[tb(&token, 1)]));
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(0), DenyReason::BalanceUnavailable)
    );
}

#[test]
fn a_token_returning_the_wrong_type_is_balance_unavailable() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);
    let wide = env.register(WideBalance, ()); // returns u64, not i128

    let id = c.create(&owner, &list(&env, &[tb(&wide, 1)]));
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(0), DenyReason::BalanceUnavailable)
    );
}

#[test]
fn a_contract_without_a_balance_function_is_balance_unavailable() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);

    // The policy contract itself is a deployed contract with no `balance` function.
    let id = c.create(&owner, &list(&env, &[tb(&c.address, 1)]));
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(0), DenyReason::BalanceUnavailable)
    );
}

#[test]
fn works_with_a_stellar_asset_contract() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let holder = Address::generate(&env);
    let other = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    StellarAssetClient::new(&env, &sac.address()).mint(&holder, &500);

    let id = c.create(&owner, &list(&env, &[tb(&sac.address(), 500)]));
    assert_eq!(c.evaluate(&id, &holder), allowed(1));
    assert_eq!(
        c.evaluate(&id, &other),
        denied(1, Some(0), DenyReason::BelowMinimum)
    );
    // Not covered here: a classic account with no trustline. A real asset contract raises an error for it
    // (Testnet, Spike 0), which this contract reports as BalanceUnavailable, same as PanicToken above.
}

// ---------------------------------------------------------------- nft balance

#[test]
fn nft_balance_boundaries() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let (none, one, two) = (
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    );
    let collection = env.register(MockNft, ());
    let n = MockNftClient::new(&env, &collection);
    n.set_balance(&one, &1);
    n.set_balance(&two, &2);

    let id = c.create(&owner, &list(&env, &[nb(&collection, 2)]));
    assert_eq!(
        c.evaluate(&id, &none),
        denied(1, Some(0), DenyReason::BelowMinimum)
    );
    assert_eq!(
        c.evaluate(&id, &one),
        denied(1, Some(0), DenyReason::BelowMinimum)
    );
    assert_eq!(c.evaluate(&id, &two), allowed(1));
}

#[test]
fn an_nft_balance_of_another_width_is_balance_unavailable() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);
    let wide = env.register(WideBalance, ()); // u64
    let fungible = token_with(&env, &who, 100); // i128

    let id = c.create(&owner, &list(&env, &[nb(&wide, 1)]));
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(0), DenyReason::BalanceUnavailable)
    );
    let id = c.create(&owner, &list(&env, &[nb(&fungible, 1)]));
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(0), DenyReason::BalanceUnavailable)
    );
}

// ---------------------------------------------------------------- time window

#[test]
fn time_window_is_inclusive_at_the_start_and_exclusive_at_the_end() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);
    let id = c.create(&owner, &list(&env, &[tw(Some(100), Some(200))]));

    set_time(&env, 99);
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(0), DenyReason::BeforeWindow)
    );
    set_time(&env, 100);
    assert_eq!(c.evaluate(&id, &who), allowed(1));
    set_time(&env, 199);
    assert_eq!(c.evaluate(&id, &who), allowed(1));
    set_time(&env, 200);
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(0), DenyReason::AfterWindow)
    );
}

#[test]
fn open_ended_time_windows() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);
    let from_only = c.create(&owner, &list(&env, &[tw(Some(100), None)]));
    let to_only = c.create(&owner, &list(&env, &[tw(None, Some(100))]));

    set_time(&env, 0);
    assert_eq!(
        c.evaluate(&from_only, &who),
        denied(1, Some(0), DenyReason::BeforeWindow)
    );
    assert_eq!(c.evaluate(&to_only, &who), allowed(1));
    set_time(&env, u64::MAX);
    assert_eq!(c.evaluate(&from_only, &who), allowed(1));
    assert_eq!(
        c.evaluate(&to_only, &who),
        denied(1, Some(0), DenyReason::AfterWindow)
    );
}

// ---------------------------------------------------------------- AND composition

#[test]
fn all_conditions_must_hold() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);
    let token = token_with(&env, &who, 10);
    let nft = nft_with(&env, &who, 1);
    set_time(&env, 500);

    let id = c.create(
        &owner,
        &list(
            &env,
            &[tb(&token, 10), nb(&nft, 1), tw(Some(100), Some(1000))],
        ),
    );
    assert_eq!(c.evaluate(&id, &who), allowed(1));
}

#[test]
fn the_failed_index_is_the_first_failing_condition_in_order() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);
    let passes = token_with(&env, &who, 10);
    let fails = token_with(&env, &who, 0);
    set_time(&env, 0); // a window starting at 1000 also fails

    let late_window = tw(Some(1000), None);
    let first = c.create(
        &owner,
        &list(&env, &[tb(&passes, 5), tb(&fails, 5), late_window.clone()]),
    );
    assert_eq!(
        c.evaluate(&first, &who),
        denied(1, Some(1), DenyReason::BelowMinimum)
    );

    let second = c.create(
        &owner,
        &list(&env, &[late_window, tb(&passes, 5), tb(&fails, 5)]),
    );
    assert_eq!(
        c.evaluate(&second, &who),
        denied(1, Some(0), DenyReason::BeforeWindow)
    );
}

#[test]
fn evaluation_stops_at_the_first_failure() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let who = Address::generate(&env);
    let fails = token_with(&env, &who, 0);
    let later = token_with(&env, &who, 100);
    let later_reads = MockTokenClient::new(&env, &later);

    let id = c.create(&owner, &list(&env, &[tb(&fails, 5), tb(&later, 5)]));
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(0), DenyReason::BelowMinimum)
    );
    assert_eq!(
        later_reads.calls(),
        0,
        "a condition after the first failure must not be evaluated"
    );

    let id = c.create(&owner, &list(&env, &[tb(&later, 5), tb(&fails, 5)]));
    assert_eq!(
        c.evaluate(&id, &who),
        denied(1, Some(1), DenyReason::BelowMinimum)
    );
    assert_eq!(later_reads.calls(), 1);
}

// ---------------------------------------------------------------- lifetime

#[test]
fn writes_extend_the_policy_lifetime_and_anyone_can_bump() {
    let env = new_env();
    let c = client(&env);
    let owner = Address::generate(&env);
    let id = c.create(&owner, &list(&env, &[tw(Some(1), None)]));
    assert!(policy_ttl(&env, &c, id) >= TTL_EXTEND_TO);

    // 70 days later the policy is below the 30-day top-up threshold, so a write or a bump is needed.
    env.ledger()
        .with_mut(|l| l.sequence_number += 70 * DAY_IN_LEDGERS);
    assert!(policy_ttl(&env, &c, id) < TTL_THRESHOLD);

    env.mock_auths(&[]); // bump needs no authorization
    c.bump(&id);
    assert!(policy_ttl(&env, &c, id) >= TTL_EXTEND_TO);
}
