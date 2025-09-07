#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{ testutils::{Address as TestAddress, Ledger}, contract, contractimpl, Address, Env, String };


// ---- Mock Oracle used by tests ----
#[contract]
struct MockOracle;
#[contractimpl]
impl MockOracle {
    pub fn get_spot(env: Env, _pair: String, _override_max: Option<u64>) -> (i128, u32, u64) {
        let now = env.ledger().timestamp();
        // deterministic, increasing-ish value
        let price = 1_000_000_00i128 + ((now % 10) as i128) * 1_000_000i128;
        (price, 8u32, now)
    }
}

// Helpers
fn setup_auth() -> (Env, RoundEngineClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.ledger().with_mut(|l| {
        l.timestamp = 1_700_000_000;
        l.sequence_number = 1;
    });
    env.mock_all_auths(); // positive-path auth
    let re_id = env.register_contract(None, RoundEngine);
    let client = RoundEngineClient::new(&env, &re_id);

    let oracle_id = env.register_contract(None, MockOracle);
    let admin = <Address as TestAddress>::generate(&env);
    let creator = <Address as TestAddress>::generate(&env);

    client.init(&admin);
    client.set_oracle(&oracle_id);
    (env, client, admin, creator, oracle_id)
}

fn setup_plain() -> (Env, RoundEngineClient<'static>, Address, Address) {
    let env = Env::default(); // no mock_all_auths -> admin required
    env.ledger().with_mut(|l| {
        l.timestamp = 1_700_000_000;
        l.sequence_number = 1;
    });
    let re_id = env.register_contract(None, RoundEngine);
    let client = RoundEngineClient::new(&env, &re_id);

    let admin = <Address as TestAddress>::generate(&env);
    let creator = <Address as TestAddress>::generate(&env);

    client.init(&admin);
    (env, client, admin, creator)
}

// -------- Tests --------

#[test]
fn init_and_set_oracle_ok() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    // create one round quickly just to assert a happy path
    let asset = String::from_str(&env, "BTC");
    let id = client.create_round(&creator, &asset, &5u64, &5u64);
    // NOTE: the signature in lib is (creator, pair, lock_secs, duration_secs); adjust if needed:
    // let id = client.create_round(&creator, &pair, &5u64, &5u64);
    // (If you changed the signature earlier, keep that version.)
    let r = client.get_round(&id);
    assert!(matches!(r.status, RoundStatus::Created));
}

#[test]
#[should_panic]
fn set_oracle_requires_admin_auth() {
    let (env, client, _admin, _creator) = setup_plain();
    let fake_oracle = env.register_contract(None, MockOracle);
    // without mock_all_auths, require_admin should fail
    client.set_oracle(&fake_oracle);
}

#[test]
#[should_panic]
fn create_round_invalid_times_panics() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "eth/usdc");
    // lock_secs = 0 -> Err::InvalidTimes
    client.create_round(&creator, &pair, &0u64, &10u64);
}

#[test]
fn join_ok_and_counts() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "xrp/usdc");
    let id = client.create_round(&creator, &pair, &10u64, &10u64);
    let p1 = <Address as TestAddress>::generate(&env);
    let p2 = <Address as TestAddress>::generate(&env);
    client.join(&id, &p1, &1u32);
    client.join(&id, &p2, &0u32);
    let r = client.get_round(&id);
    assert_eq!(r.up_count, 1);
    assert_eq!(r.down_count, 1);
}

#[test]
#[should_panic]
fn join_after_lock_panics() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "sol/usdc");
    let id = client.create_round(&creator, &pair, &1u64, &10u64);
    env.ledger().with_mut(|l| {
        l.timestamp += 1; // now == lock_ts
        l.sequence_number += 1;
    });
    let who = <Address as TestAddress>::generate(&env);
    client.join(&id, &who, &1u32);
}

#[test]
#[should_panic]
fn join_double_panics() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "ada/usdc");
    let id = client.create_round(&creator, &pair, &10u64, &10u64);
    let who = <Address as TestAddress>::generate(&env);
    client.join(&id, &who, &1u32);
    client.join(&id, &who, &0u32); // second time -> AlreadyJoined
}

#[test]
#[should_panic]
fn lock_before_time_panics() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "dot/usdc");
    let id = client.create_round(&creator, &pair, &5u64, &5u64);
    // now < lock_ts
    client.lock(&id);
}

#[test]
#[should_panic]
fn lock_wrong_state_panics() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "arb/usdc");
    let id = client.create_round(&creator, &pair, &1u64, &5u64);
    env.ledger().with_mut(|l| { l.timestamp += 1; l.sequence_number += 1; });
    client.lock(&id);
    // second lock should fail (status != Created)
    client.lock(&id);
}

#[test]
#[should_panic]
fn settle_before_time_panics() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "op/usdc");
    let id = client.create_round(&creator, &pair, &1u64, &5u64);
    env.ledger().with_mut(|l| { l.timestamp += 1; l.sequence_number += 1; });
    client.lock(&id);
    // now < settle_ts
    client.settle(&id);
}

#[test]
#[should_panic]
fn settle_wrong_state_panics() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "sei/usdc");
    let id = client.create_round(&creator, &pair, &1u64, &1u64);
    // settle without lock -> BadState
    client.settle(&id);
}

#[test]
fn settle_ok_updates_state() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "btc/usdc");
    let id = client.create_round(&creator, &pair, &2u64, &3u64);

    // join a couple so flow looks realistic
    let a = <Address as TestAddress>::generate(&env);
    let b = <Address as TestAddress>::generate(&env);
    client.join(&id, &a, &1u32);
    client.join(&id, &b, &0u32);

    env.ledger().with_mut(|l| { l.timestamp += 2; l.sequence_number += 1; });
    client.lock(&id);

    env.ledger().with_mut(|l| { l.timestamp += 3; l.sequence_number += 1; });
    client.settle(&id);

    let r = client.get_round(&id);
    assert!(matches!(r.status, RoundStatus::Settled));
    assert!(r.lock_price.is_some());
    assert!(r.settle_price.is_some());
}

#[test]
#[should_panic]
fn cancel_requires_admin_auth() {
    let (env, client, _admin, creator) = setup_plain();
    let pair = String::from_str(&env, "link/usdc");
    let id = client.create_round(&creator, &pair, &10u64, &10u64);
    // no mock_all_auths => require_admin should fail
    client.cancel(&id);
}

#[test]
fn cancel_ok_before_settle() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "inj/usdc");
    let id = client.create_round(&creator, &pair, &10u64, &10u64);
    client.cancel(&id);
    let r = client.get_round(&id);
    assert!(matches!(r.status, RoundStatus::Canceled));
}

#[test]
#[should_panic]
fn cancel_after_settle_panics() {
    let (env, client, _admin, creator, _oracle) = setup_auth();
    let pair = String::from_str(&env, "atom/usdc");
    let id = client.create_round(&creator, &pair, &1u64, &1u64);
    env.ledger().with_mut(|l| { l.timestamp += 1; l.sequence_number += 1; });
    client.lock(&id);
    env.ledger().with_mut(|l| { l.timestamp += 1; l.sequence_number += 1; });
    client.settle(&id);
    client.cancel(&id); // -> BadState
}

#[test]
#[should_panic]
fn get_round_unknown_panics() {
    let (_env, client, _admin, _creator, _oracle) = setup_auth();
    let _ = client.get_round(&999_999u64); // not created -> RoundNotFound
}
