extern crate std;

use soroban_sdk::{
    contract, contractimpl, testutils::{Address as _, Ledger},
    Address, Env, String
};
use std::panic::AssertUnwindSafe;
// use crate::oracle_adapter::{OracleAdapter, Asset};
use super::*;

#[contract]
pub struct MockReflector;

#[contractimpl]
impl MockReflector {
    pub fn lastprice(e: Env, asset: Asset) -> (i128, u32, u64) {
        let ts = e.ledger().timestamp();
        match asset {
            Asset::Other(sym) => {
                let p = (sym.len() as i128) * 100_000_000i128; // 8 decimals
                (p, 8, ts)
            }
            Asset::Stellar(code, _issuer) => {
                let p = (code.len() as i128) * 100_000_000i128;
                (p, 8, ts)
            }
        }
    }
}

fn deploy_mock_reflector(e: &Env) -> Address {
    e.register(MockReflector, ()) // ctor has no args
}
fn deploy_adapter(e: &Env) -> Address {
    e.register(OracleAdapter, ()) // ctor has no args
}

#[test]
fn init_and_permissions() {
    let e = Env::default();
    e.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    let admin = Address::generate(&e);
    let alice = Address::generate(&e);

    let ref_addr = deploy_mock_reflector(&e);
    let adapter = deploy_adapter(&e);

    e.as_contract(&adapter, || {
        OracleAdapter::init(e.clone(), admin.clone(), ref_addr.clone());

        // admin can set feeder
        OracleAdapter::set_feeder(e.clone(), admin.clone(), alice.clone());

        // non-admin cannot set feeder
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            OracleAdapter::set_feeder(e.clone(), alice.clone(), admin.clone());
        }));
        assert!(res.is_err());

        // non-admin cannot set reflector
        let other_ref = deploy_mock_reflector(&e);
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            OracleAdapter::set_reflector(e.clone(), alice.clone(), other_ref.clone());
        }));
        assert!(res.is_err());
    });
}

#[test]
fn upsert_pull_and_get_spot_rescale() {
    let e = Env::default();
    e.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    let admin = Address::generate(&e);
    let feeder = Address::generate(&e);

    let ref_addr = deploy_mock_reflector(&e);
    let adapter = deploy_adapter(&e);

    e.as_contract(&adapter, || {
        OracleAdapter::init(e.clone(), admin.clone(), ref_addr.clone());
        OracleAdapter::set_feeder(e.clone(), admin.clone(), feeder.clone());

        let code = String::from_str(&e, "BTC");
        OracleAdapter::upsert_asset(e.clone(), admin.clone(), code.clone(), 8, 600);

        // Pull from Reflector using asset enum
        OracleAdapter::pull_from_reflector(e.clone(), feeder.clone(), Asset::Other(code.clone()));

        // Spot rescaled to 6 decimals: "BTC".len() = 3 -> price = 3e8 -> 3e6
        let (p, d, ts) = OracleAdapter::get_spot(e.clone(), code.clone(), 6);
        assert_eq!(d, 6);
        assert_eq!(ts, e.ledger().timestamp());
        assert_eq!(p, 3_000_000i128);
    });
}

#[test]
fn push_price_twap_and_staleness() {
    let e = Env::default();
    e.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    let admin = Address::generate(&e);
    let feeder = Address::generate(&e);

    let ref_addr = deploy_mock_reflector(&e);
    let adapter = deploy_adapter(&e);

    e.as_contract(&adapter, || {
        OracleAdapter::init(e.clone(), admin.clone(), ref_addr.clone());
        OracleAdapter::set_feeder(e.clone(), admin.clone(), feeder.clone());

        let code = String::from_str(&e, "ETH");
        OracleAdapter::upsert_asset(e.clone(), admin.clone(), code.clone(), 8, 10);

        // Manually push three points
        OracleAdapter::push_price(e.clone(), feeder.clone(), code.clone(), 100_000_000, 8, 100);
        OracleAdapter::push_price(e.clone(), feeder.clone(), code.clone(), 200_000_000, 8, 101);
        OracleAdapter::push_price(e.clone(), feeder.clone(), code.clone(), 300_000_000, 8, 102);

        // Fresh window
        e.ledger().with_mut(|l| l.timestamp = 105);
        let (twap, d, ts) = OracleAdapter::get_twap(e.clone(), code.clone(), 3, 8);
        assert_eq!(d, 8);
        assert_eq!(ts, 102);
        assert_eq!(twap, 200_000_000);

        // Stale now
        e.ledger().with_mut(|l| l.timestamp = 200);
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            OracleAdapter::get_spot(e.clone(), code.clone(), 8);
        }));
        assert!(res.is_err());
    });
}

#[test]
fn history_cap_trim_and_large_twap_request() {
    let e = Env::default();
    e.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    let admin = Address::generate(&e);
    let feeder = Address::generate(&e);

    let ref_addr = deploy_mock_reflector(&e);
    let adapter = deploy_adapter(&e);

    e.as_contract(&adapter, || {
        OracleAdapter::init(e.clone(), admin.clone(), ref_addr.clone());
        OracleAdapter::set_feeder(e.clone(), admin.clone(), feeder.clone());

        let code = String::from_str(&e, "XLM");
        OracleAdapter::upsert_asset(e.clone(), admin.clone(), code.clone(), 7, 600);

        // Insert > HISTORY_CAP entries
        let cap: u32 = 256;
        for i in 0..(cap + 10) {
            OracleAdapter::push_price(e.clone(), feeder.clone(), code.clone(), 1_000_000, 7, i as u64);
        }

        // TWAP should clamp to available and return newest ts
        e.ledger().with_mut(|l| l.timestamp = (cap + 10) as u64);
        let (_twap, d, ts) = OracleAdapter::get_twap(e.clone(), code.clone(), cap + 999, 7);
        assert_eq!(d, 7);
        assert_eq!(ts, (cap + 10 - 1) as u64);
    });
}

#[test]
fn error_cases_unknown_asset_nohistory_rounding_and_auth() {
    let e = Env::default();
    e.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
    let admin = Address::generate(&e);
    let feeder = Address::generate(&e);
    let intruder = Address::generate(&e);

    let ref_addr = deploy_mock_reflector(&e);
    let adapter = deploy_adapter(&e);

    e.as_contract(&adapter, || {
        OracleAdapter::init(e.clone(), admin.clone(), ref_addr.clone());
        OracleAdapter::set_feeder(e.clone(), admin.clone(), feeder.clone());

        // Unknown asset -> get_spot should panic
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            OracleAdapter::get_spot(e.clone(), String::from_str(&e, "UNKNOWN"), 8);
        }));
        assert!(res.is_err());

        // Configure but no history -> get_spot should panic
        let code = String::from_str(&e, "ROUND");
        OracleAdapter::upsert_asset(e.clone(), admin.clone(), code.clone(), 1, 1000);
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            OracleAdapter::get_spot(e.clone(), code.clone(), 0);
        }));
        assert!(res.is_err());

        // Rounding half-up check: push price 15 with decimals=1; out_decimals=0 => 2
        OracleAdapter::push_price(e.clone(), feeder.clone(), code.clone(), 15, 1, 123);
        e.ledger().with_mut(|l| l.timestamp = 124);
        let (p0, d0, _ts0) = OracleAdapter::get_spot(e.clone(), code.clone(), 0);
        assert_eq!(d0, 0);
        assert_eq!(p0, 2);

        // Non-feeder push should panic
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            OracleAdapter::push_price(e.clone(), intruder.clone(), code.clone(), 42, 1, 125);
        }));
        assert!(res.is_err());

        // Bad decimals in upsert should panic
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            OracleAdapter::upsert_asset(e.clone(), admin.clone(), String::from_str(&e, "BAD"), 50, 10);
        }));
        assert!(res.is_err());

        // get_twap with 0 records should panic
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            OracleAdapter::get_twap(e.clone(), code.clone(), 0, 0);
        }));
        assert!(res.is_err());
    });
}
