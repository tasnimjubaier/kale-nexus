#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as TestAddress, Address, Env};

// Helpers
fn setup_env_auth() -> (
    Env,
    CreatorHubClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths(); // allow admin-gated calls to pass
    let id = env.register_contract(None, CreatorHub);
    let c = CreatorHubClient::new(&env, &id);

    let admin = <Address as TestAddress>::generate(&env);
    let platform = <Address as TestAddress>::generate(&env);
    let creator = <Address as TestAddress>::generate(&env);
    let stakers = <Address as TestAddress>::generate(&env);

    c.init(&admin);
    (env, c, admin, platform, creator, stakers)
}

fn setup_env_plain() -> (
    Env,
    CreatorHubClient<'static>,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default(); // NO mock_all_auths -> auth will be enforced
    let id = env.register_contract(None, CreatorHub);
    let c = CreatorHubClient::new(&env, &id);

    let admin = <Address as TestAddress>::generate(&env);
    let platform = <Address as TestAddress>::generate(&env);
    let creator = <Address as TestAddress>::generate(&env);
    let stakers = <Address as TestAddress>::generate(&env);

    c.init(&admin);
    (env, c, admin, platform, creator, stakers)
}

// --- Tests ---

#[test]
fn credit_splits_and_claim() {
    let (_env, c, _admin, platform, creator, stakers) = setup_env_auth();

    // 600 splits into 300/200/100  (3/2/1 of 6)
    c.credit_fees(&platform, &creator, &stakers, &600i128);
    assert_eq!(c.balance_of(&platform), 300);
    assert_eq!(c.balance_of(&creator), 200);
    assert_eq!(c.balance_of(&stakers), 100);

    // Claim resets only the claimer
    let got = c.claim(&creator);
    assert_eq!(got, 200);
    assert_eq!(c.balance_of(&creator), 0);
    assert_eq!(c.balance_of(&platform), 300);
    assert_eq!(c.balance_of(&stakers), 100);
}

#[test]
fn credit_rounding_and_negatives() {
    let (_env, c, _admin, platform, creator, stakers) = setup_env_auth();

    // total_fee = 7 -> sixth=1 => platform=3, creator=2, stakers gets the rest: 2
    c.credit_fees(&platform, &creator, &stakers, &7i128);
    assert_eq!(c.balance_of(&platform), 3);
    assert_eq!(c.balance_of(&creator), 2);
    assert_eq!(c.balance_of(&stakers), 2);

    // negative -> treated as 0
    c.credit_fees(&platform, &creator, &stakers, &(-10i128));
    assert_eq!(c.balance_of(&platform), 3);
    assert_eq!(c.balance_of(&creator), 2);
    assert_eq!(c.balance_of(&stakers), 2);
}

#[test]
fn multiple_credits_accumulate_then_claim_all() {
    let (_env, c, _admin, platform, creator, stakers) = setup_env_auth();

    c.credit_fees(&platform, &creator, &stakers, &60i128); // 30 / 20 / 10
    c.credit_fees(&platform, &creator, &stakers, &12i128); //  6 /  4 /  2
    assert_eq!(c.balance_of(&platform), 36);
    assert_eq!(c.balance_of(&creator), 24);
    assert_eq!(c.balance_of(&stakers), 12);

    // claim stakers only
    assert_eq!(c.claim(&stakers), 12);
    assert_eq!(c.balance_of(&stakers), 0);
}

#[test]
fn balance_of_new_user_is_zero() {
    let (env, c, _admin, _p, _cr, _s) = setup_env_auth();
    let outsider = <Address as TestAddress>::generate(&env);
    assert_eq!(c.balance_of(&outsider), 0);
}

#[test]
#[should_panic] // must not allow double init
fn init_only_once_panics() {
    let (_env, c, admin, _p, _cr, _s) = setup_env_auth();
    c.init(&admin); // -> panic AlreadyInitialized
}

#[test]
#[should_panic] // admin auth should be enforced when auth isn't mocked
fn credit_requires_admin_auth() {
    let (_env, c, _admin, platform, creator, stakers) = setup_env_plain();
    c.credit_fees(&platform, &creator, &stakers, &1i128); // -> panic
}

#[test]
fn large_values_do_not_panic() {
    let (_env, c, _admin, platform, creator, stakers) = setup_env_auth();

    // Very large fee to exercise saturating_add path; ensure no panic
    let huge = i128::MAX / 2;
    c.credit_fees(&platform, &creator, &stakers, &huge);
    let sum = c.balance_of(&platform) + c.balance_of(&creator) + c.balance_of(&stakers);
    assert!(sum > 0);
}
