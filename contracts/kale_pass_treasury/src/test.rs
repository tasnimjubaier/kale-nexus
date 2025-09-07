#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{
    testutils::Address as TestAddress,
    vec,
    Address, Env, Vec,
};

// ----------------- helpers -----------------

fn setup_auth() -> (Env, KalePassTreasuryClient<'static>, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths(); // allow admin-gated calls without signatures

    let id = env.register_contract(None, KalePassTreasury);
    let c = KalePassTreasuryClient::new(&env, &id);

    let admin   = <Address as TestAddress>::generate(&env);
    let user_a  = <Address as TestAddress>::generate(&env);
    let user_b  = <Address as TestAddress>::generate(&env);
    let user_c  = <Address as TestAddress>::generate(&env);

    c.init(&admin);
    (env, c, admin, user_a, user_b, user_c)
}

fn setup_plain() -> (Env, KalePassTreasuryClient<'static>, Address, Address) {
    let env = Env::default(); // no mock_all_auths -> require_auth() is enforced
    let id = env.register_contract(None, KalePassTreasury);
    let c = KalePassTreasuryClient::new(&env, &id);

    let admin  = <Address as TestAddress>::generate(&env);
    let user_x = <Address as TestAddress>::generate(&env);

    c.init(&admin);
    (env, c, admin, user_x)
}

// ----------------- tests -----------------

#[test]
fn default_tiers_and_get_discount() {
    let (env, c, _admin, u1, u2, u3) = setup_auth();

    // Default tiers: 0 -> 0%, 100 -> 20%, 500 -> 40%
    c.admin_set_stake(&u1, &0u128);     // below first nonzero tier
    c.admin_set_stake(&u2, &100u128);   // exactly 100
    c.admin_set_stake(&u3, &500u128);   // exactly 500

    assert_eq!(c.get_discount_bps(&u1), 0);
    assert_eq!(c.get_discount_bps(&u2), 2000);
    assert_eq!(c.get_discount_bps(&u3), 4000);

    // Between thresholds should pick the best matching tier
    let u4 = <Address as TestAddress>::generate(&env);
    c.admin_set_stake(&u4, &499u128);
    assert_eq!(c.get_discount_bps(&u4), 2000);

    let u5 = <Address as TestAddress>::generate(&env);
    c.admin_set_stake(&u5, &5_000u128);
    assert_eq!(c.get_discount_bps(&u5), 4000);
}

#[test]
fn set_tiers_valid_then_effective_discounts() {
    let (env, c, _admin, u1, u2, u3) = setup_auth();

    // New valid tier schedule (monotonic thresholds; <= 10_000 bps)
    let new_tiers: Vec<Tier> = vec![
        &env,
        Tier { threshold: 0,    discount_bps: 1000 }, // 10%
        Tier { threshold: 200,  discount_bps: 2500 }, // 25%
        Tier { threshold: 1000, discount_bps: 5000 }, // 50%
    ];
    c.set_tiers(&new_tiers);

    c.admin_set_stake(&u1, &0u128);
    c.admin_set_stake(&u2, &200u128);
    c.admin_set_stake(&u3, &1500u128);

    assert_eq!(c.get_discount_bps(&u1), 1000); // 10%
    assert_eq!(c.get_discount_bps(&u2), 2500); // 25%
    assert_eq!(c.get_discount_bps(&u3), 5000); // 50%

    // A user between 200 and 1000 gets 25%
    let u_mid = <Address as TestAddress>::generate(&env);
    c.admin_set_stake(&u_mid, &600u128);
    assert_eq!(c.get_discount_bps(&u_mid), 2500);
}

#[test]
#[should_panic] // thresholds must be non-decreasing
fn set_tiers_monotonicity_panics() {
    let (env, c, _admin, _u1, _u2, _u3) = setup_auth();
    let bad: Vec<Tier> = vec![
        &env,
        Tier { threshold: 100, discount_bps: 1000 },
        Tier { threshold: 50,  discount_bps: 2000 }, // decreasing -> panic Err::BadInput
    ];
    c.set_tiers(&bad);
}

#[test]
#[should_panic] // discount_bps must be <= 10000
fn set_tiers_bps_ceiling_panics() {
    let (env, c, _admin, _u1, _u2, _u3) = setup_auth();
    let bad: Vec<Tier> = vec![
        &env,
        Tier { threshold: 0, discount_bps: 10001 }, // > 100%
    ];
    c.set_tiers(&bad);
}

#[test]
fn admin_set_stake_and_read_back_via_discount() {
    let (_env, c, _admin, u1, _u2, _u3) = setup_auth();
    c.admin_set_stake(&u1, &42u128);
    // with default tiers, 42 -> 0%
    assert_eq!(c.get_discount_bps(&u1), 0);
    c.admin_set_stake(&u1, &500u128);
    assert_eq!(c.get_discount_bps(&u1), 4000);
}

#[test]
#[should_panic] // must not allow double init
fn init_only_once_panics() {
    let (_env, c, admin, _u1, _u2, _u3) = setup_auth();
    c.init(&admin); // Err::AlreadyInitialized
}

#[test]
#[should_panic] // admin auth enforced when we don't mock
fn admin_set_stake_requires_admin_auth() {
    let (_env, c, _admin, user) = setup_plain();
    c.admin_set_stake(&user, &10u128); // require_auth -> panic
}

#[test]
#[should_panic] // admin auth enforced when we don't mock
fn set_tiers_requires_admin_auth() {
    let (env, c, _admin, _user) = setup_plain();
    let tiers: Vec<Tier> = vec![
        &env,
        Tier { threshold: 0,   discount_bps: 0 },
        Tier { threshold: 100, discount_bps: 1000 },
    ];
    c.set_tiers(&tiers); // require_auth -> panic
}

#[test]
fn large_values_do_not_panic() {
    let (env, c, _admin, whale, _u2, _u3) = setup_auth();

    // Push high thresholds and maximum discount
    let tiers: Vec<Tier> = vec![
        &env,
        Tier { threshold: 0,          discount_bps: 0 },
        Tier { threshold: 1_000_000,  discount_bps: 10_000 }, // 100%
    ];
    c.set_tiers(&tiers);

    c.admin_set_stake(&whale, &1_000_000_000_000_000_000u128);
    assert_eq!(c.get_discount_bps(&whale), 10_000);
}
