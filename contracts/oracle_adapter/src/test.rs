#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as TestAddress, Ledger},
    Address, Env, String,
};

// Create a client bound to this Env
fn register(env: &Env) -> ContractClient<'_> {
    // OK to keep the deprecated API for now (warning only)
    let id = env.register_contract(None, Contract);
    ContractClient::new(env, &id)
}

// Deterministic Env + mocked auths (for positive-path tests)
fn setup_env() -> (Env, Address, Address) {
    let env = Env::default();
    env.ledger().with_mut(|l| {
        l.timestamp = 1_700_000_000;
        l.sequence_number = 1;
    });
    env.mock_all_auths();
    // let admin = TestAddress::generate(&env);
    // let feeder = TestAddress::generate(&env);
    let admin = <Address as TestAddress>::generate(&env);
    let feeder = <Address as TestAddress>::generate(&env);
    (env, admin, feeder)
}

// Deterministic Env WITHOUT mocked auths (for simple panics like not-initialized)
fn setup_env_plain() -> (Env, Address, Address) {
    let env = Env::default();
    env.ledger().with_mut(|l| {
        l.timestamp = 1_700_000_000;
        l.sequence_number = 1;
    });
    // let admin = TestAddress::generate(&env);
    // let feeder = TestAddress::generate(&env);
    let admin = <Address as TestAddress>::generate(&env);
    let feeder = <Address as TestAddress>::generate(&env);
    (env, admin, feeder)
}

#[test]
fn upsert_pair_and_spot_ok() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);

    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "btc/usdc");
    client.upsert_pair(&pair, &8u32, &3600u64);

    let ts = env.ledger().timestamp();
    client.poke(&pair, &50_000_000_000i128, &8u32, &Some(ts));

    let (p, dec, ts_out) = client.get_spot(&pair, &None);
    assert_eq!(dec, 8);
    assert_eq!(ts_out, ts);
    assert!(p > 0);
}

#[test]
#[should_panic] // init twice must panic with AlreadyInitialized
fn init_only_once_panics() {
    let (env, admin, _feeder) = setup_env();
    let client = register(&env);
    client.init(&admin);
    client.init(&admin); // <- panic
}

#[test]
#[should_panic] // calling set_feeder before init should panic (NotInitialized)
fn set_feeder_before_init_panics() {
    let (env, _admin, feeder) = setup_env_plain();
    let client = register(&env);
    client.set_feeder(&feeder); // <- panic
}

#[test]
#[should_panic] // stale guard must panic
fn spot_staleness_guard_panics() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);

    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "eth/usdc");
    client.upsert_pair(&pair, &8u32, &30u64);

    let now = env.ledger().timestamp();
    client.poke(&pair, &2_000_000_000i128, &8u32, &Some(now));

    env.ledger().with_mut(|l| {
        l.timestamp = now + 31;
        l.sequence_number += 1;
    });

    let _ = client.get_spot(&pair, &None); // <- panic (stale)
}

#[test]
fn spot_future_ts_does_not_underflow() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);

    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "btc/usdc");
    client.upsert_pair(&pair, &8u32, &600u64);

    let now = env.ledger().timestamp();
    client.poke(&pair, &42_000_000i128, &8u32, &Some(now + 120));

    let (_p, dec, _ts) = client.get_spot(&pair, &None);
    assert_eq!(dec, 8);
}

#[test]
#[should_panic] // window 0 must panic with EmptyWindow
fn twap_window_zero_panics() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);

    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "xrp/usdc");
    client.upsert_pair(&pair, &6u32, &3600u64);

    let _ = client.get_twap(&pair, &0u64, &None); // <- panic
}

#[test]
fn twap_equal_weight_rescales_to_cfg_decimals() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);

    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "sol/usdc");
    client.upsert_pair(&pair, &8u32, &600u64);

    let t0 = env.ledger().timestamp();

    // pt1: 6 decimals -> upscale to 8
    env.ledger().with_mut(|l| {
        l.timestamp = t0 + 1;
        l.sequence_number += 1;
    });
    client.poke(&pair, &150_000_00i128, &6u32, &Some(env.ledger().timestamp()));

    // pt2: 8 decimals
    env.ledger().with_mut(|l| {
        l.timestamp = t0 + 2;
        l.sequence_number += 1;
    });
    client.poke(&pair, &151_000_0000i128, &8u32, &Some(env.ledger().timestamp()));

    // pt3: 10 decimals -> downscale to 8
    env.ledger().with_mut(|l| {
        l.timestamp = t0 + 3;
        l.sequence_number += 1;
    });
    client.poke(&pair, &149_500_000_000i128, &10u32, &Some(env.ledger().timestamp()));

    let (avg, dec, start, end) = client.get_twap(&pair, &120u64, &None);
    assert_eq!(dec, 8);
    assert!(avg > 0);
    assert!(end >= start);
}

#[test]
fn twap_start_saturates_when_window_gt_now() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);

    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "ada/usdc");
    client.upsert_pair(&pair, &6u32, &3600u64);

    client.poke(&pair, &1_000_000i128, &6u32, &None);
    let (_avg, d, start, end) = client.get_twap(&pair, &u64::MAX, &None);
    assert_eq!(d, 6);
    assert!(start <= end);
}

#[test]
fn history_trims_to_cap() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);

    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "doge/usdc");
    client.upsert_pair(&pair, &4u32, &3600u64);

    for _ in 0..(HISTORY_CAP + 20) {
        env.ledger().with_mut(|l| {
            l.timestamp += 1;
            l.sequence_number += 1;
        });
        client.poke(&pair, &1_0000i128, &4u32, &Some(env.ledger().timestamp()));
    }

    let (_p, dec, _ts) = client.get_spot(&pair, &None);
    assert_eq!(dec, 4);
}

#[test]
#[should_panic] // unknown pair -> get_spot panics
fn unknown_pair_spot_panics() {
    let (env, _admin, _feeder) = setup_env();
    let client = register(&env);
    let pair = String::from_str(&env, "nope/usdc");
    let _ = client.get_spot(&pair, &None); // <- panic
}

#[test]
#[should_panic] // unknown pair -> get_twap panics
fn unknown_pair_twap_panics() {
    let (env, _admin, _feeder) = setup_env();
    let client = register(&env);
    let pair = String::from_str(&env, "nope/usdc");
    let _ = client.get_twap(&pair, &60u64, &None); // <- panic
}



#[test]
#[should_panic] // poke before set_feeder should panic
fn poke_before_set_feeder_panics() {
    let (env, admin, feeder) = setup_env_plain();
    let client = register(&env);
    client.init(&admin);
    let pair = String::from_str(&env, "btc/usdc");
    client.upsert_pair(&pair, &8u32, &3600u64);
    client.poke(&pair, &1_000_000i128, &8u32, &None); // <- panic: feeder not set / not authed
}

#[test]
#[should_panic] // poke unknown pair should panic
fn poke_unknown_pair_panics() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);
    client.init(&admin);
    client.set_feeder(&feeder);
    let pair = String::from_str(&env, "nope/usdc");
    client.poke(&pair, &1_000_000i128, &8u32, &None); // <- Err::UnknownPair via get_cfg()
}

#[test]
fn spot_rescales_when_decimals_mismatch() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);
    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "eth/usdc");
    client.upsert_pair(&pair, &8u32, &3600u64);
    // feed at 6 decimals, spot should return at 8
    client.poke(&pair, &1_234_560i128, &6u32, &None);
    let (p, d, _) = client.get_spot(&pair, &None);
    assert_eq!(d, 8);
    // 1234.56 at 6d -> 123456000 at 8d
    assert_eq!(p, 123_456_000i128);
}

#[test]
#[should_panic] // window contains no points -> Err::NoData
fn twap_no_points_in_window_panics() {
    let (env, admin, feeder) = setup_env();
    let client = register(&env);
    client.init(&admin);
    client.set_feeder(&feeder);

    let pair = String::from_str(&env, "sol/usdc");
    client.upsert_pair(&pair, &8u32, &3600u64);

    // feed a point far in the past
    let t0 = env.ledger().timestamp();
    client.poke(&pair, &10_000_000_00i128, &8u32, &Some(t0 - 1000));
    let _ = client.get_twap(&pair, &60u64, &None); // <- panic
}
