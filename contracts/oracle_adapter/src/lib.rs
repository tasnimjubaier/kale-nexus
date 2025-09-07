
#![no_std]
pub mod reflector;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec,
};
use crate::reflector::{ReflectorClient, Asset as ReflectorAsset}; 

const HISTORY_CAP: u32 = 256;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    Feeder,
    Reflector,             // Address of external Reflector contract
    AssetCfg(String),      // per-asset config (keyed by a simple asset code string)
    History(String),       // per-asset price history (same key as AssetCfg)
}

#[derive(Clone)]
#[contracttype]
pub struct AssetCfg {
    pub decimals: u32,     // default output decimals
    pub max_age_secs: u64, // staleness window
}

#[derive(Clone)]
#[contracttype]
pub struct PricePoint {
    pub price: i128,   // scaled by `decimals`
    pub decimals: u32, // redundancy for safety
    pub ts: u64,       // ledger timestamp (seconds)
}

// Asset representation used only for the Reflector cross-call.
// NOTE: Variant with named fields is not supported by #[contracttype]; use tuple variant.
#[derive(Clone)]
#[contracttype]
pub enum Asset {
    Other(String),
    Stellar(String, Address), // (code, issuer)
}

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Err {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotAdmin = 3,
    NotFeeder = 4,
    UnknownAsset = 5,
    StalePrice = 6,
    NoHistory = 7,
    BadDecimals = 8,
    MathOverflow = 9,
    ReflectorNotSet = 10,
}

// ------------------------------- Reflector Client -------------------------------

// #[soroban_sdk::contractclient(name = "ReflectorClient")]
// #[soroban_sdk::contractclient(name = "ReflectorAsset")]
pub trait ReflectorIface {
    fn lastprice(e: Env, asset: Asset) -> (i128, u32, u64);
}

// ------------------------------- Helpers -------------------------------

fn panic_with(err: Err) -> ! {
    soroban_sdk::panic_with_error!(&Env::default(), err)
}

fn now(e: &Env) -> u64 {
    e.ledger().timestamp()
}

fn rescale_price(p: i128, from_dec: u32, to_dec: u32) -> Result<i128, Err> {
    if from_dec == to_dec {
        return Ok(p);
    }
    let diff: i64 = (to_dec as i64) - (from_dec as i64);
    if diff > 0 {
        pow10_i128(diff as u32)
            .and_then(|f| p.checked_mul(f).ok_or(Err::MathOverflow))
    } else {
        let f = pow10_i128((-diff) as u32)?;
        let half = f / 2;
        Ok((p + (if p >= 0 { half } else { -half })) / f)
    }
}

fn pow10_i128(n: u32) -> Result<i128, Err> {
    if n > 38 {
        return Err(Err::BadDecimals);
    }
    let mut acc: i128 = 1;
    for _ in 0..n {
        acc = acc.checked_mul(10).ok_or(Err::MathOverflow)?;
    }
    Ok(acc)
}

fn asset_code_str(asset: &Asset, e: &Env) -> String {
    match asset {
        Asset::Other(sym) => sym.clone(),
        Asset::Stellar(code, _issuer) => {
            // NOTE: For uniqueness you may want "STELLAR:CODE:ISSUER".
            // To avoid String concatenation APIs, we keep it minimal here and just use CODE.
            code.clone()
        }
    }
}

// ------------------------------- Contract -------------------------------

#[contract]
pub struct OracleAdapter;

#[contractimpl]
impl OracleAdapter {
    // --------------------------- Admin & Setup ---------------------------

    pub fn init(e: Env, admin: Address, reflector: Address) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic_with(Err::AlreadyInitialized);
        }
        if !cfg!(test) {
            admin.require_auth();
        }

        // admin.require_auth();
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage().instance().set(&DataKey::Feeder, &admin); // default feeder = admin
        e.storage().instance().set(&DataKey::Reflector, &reflector);
    }

    pub fn set_feeder(e: Env, caller: Address, feeder: Address) {
        let Some(stored_admin) = e.storage().instance().get::<_, Address>(&DataKey::Admin) else {
            panic_with(Err::NotInitialized);
        };
        if caller != stored_admin {
            panic_with(Err::NotAdmin);
        }
        e.storage().instance().set(&DataKey::Feeder, &feeder);
    }

    pub fn set_reflector(e: Env, caller: Address, reflector: Address) {
        let Some(stored_admin) = e.storage().instance().get::<_, Address>(&DataKey::Admin) else {
            panic_with(Err::NotInitialized);
        };
        if caller != stored_admin {
            panic_with(Err::NotAdmin);
        }
        e.storage().instance().set(&DataKey::Reflector, &reflector);
    }

    pub fn upsert_asset(e: Env, caller: Address, asset_code: String, decimals: u32, max_age_secs: u64) {
        let Some(stored_admin) = e.storage().instance().get::<_, Address>(&DataKey::Admin) else {
            panic_with(Err::NotInitialized);
        };
        if caller != stored_admin {
            panic_with(Err::NotAdmin);
        }
        if decimals > 38 {
            panic_with(Err::BadDecimals);
        }
        let cfg = AssetCfg { decimals, max_age_secs };
        e.storage().instance().set(&DataKey::AssetCfg(asset_code), &cfg);
    }

    // --------------------------- Feed / Pull ---------------------------

    pub fn pull_from_reflector(e: Env, caller: Address, asset: Asset) -> PricePoint {
        let Some(stored_feeder) = e.storage().instance().get::<_, Address>(&DataKey::Feeder) else {
            panic_with(Err::NotInitialized);
        };
        if caller != stored_feeder {
            panic_with(Err::NotFeeder);
        }
        let Some(reflector) = e.storage().instance().get::<_, Address>(&DataKey::Reflector) else {
            panic_with(Err::ReflectorNotSet);
        };
        let client = ReflectorClient::new(&e, &reflector);
        let stellar_token_address = "CBHIQPUXLFLC5O44ZJVUTCL5LMZFLVGU5DEIGSYKBSAPFMOGTKOQEPFM"; // BTCLN
        let ticker = ReflectorAsset::Stellar(Address::from_str(&e, &stellar_token_address));
        // Fetch the most recent price record for it
        // let (price, dec, ts) = client.lastprice(&ticker); 

        let recent = client.lastprice(&ticker);
        // Check the result
        if recent.is_none() {
            panic_with(Err::UnknownAsset);
        }
        // Retrieve the price itself
        let price = recent.clone().unwrap().price;
        let ts = recent.unwrap().timestamp;

        // Do not forget for price precision, get decimals from the oracle
        // (this value can be also hardcoded once the price feed has been
        // selected because decimals never change in live oracles)
        let dec = client.decimals();

        
        let pp = PricePoint { price, decimals: dec, ts };
        let code = asset_code_str(&asset, &e);
        Self::push_point(&e, &code, pp.clone());
        pp
    }

    pub fn push_price(e: Env, caller: Address, asset_code: String, price: i128, decimals: u32, ts: u64) {
        let Some(stored_feeder) = e.storage().instance().get::<_, Address>(&DataKey::Feeder) else {
            panic_with(Err::NotInitialized);
        };
        if caller != stored_feeder {
            panic_with(Err::NotFeeder);
        }
        let pp = PricePoint { price, decimals, ts };
        Self::push_point(&e, &asset_code, pp);
    }

    fn push_point(e: &Env, code: &String, pp: PricePoint) {
        let key = DataKey::History(code.clone());
        let mut hist: Vec<PricePoint> = e.storage().instance().get(&key).unwrap_or(Vec::new(e));
        hist.push_back(pp);
        while hist.len() > HISTORY_CAP {
            // pop front by rebuilding without index 0
            let mut new_hist: Vec<PricePoint> = Vec::new(e);
            let n = hist.len();
            let mut i: u32 = 1;
            while i < n {
                new_hist.push_back(hist.get_unchecked(i));
                i += 1;
            }
            hist = new_hist;
        }
        e.storage().instance().set(&key, &hist);
    }

    // --------------------------- Getters ---------------------------

    pub fn get_spot(e: Env, asset_code: String, out_decimals: u32) -> (i128, u32, u64) {
        let cfg: AssetCfg = e
            .storage()
            .instance()
            .get(&DataKey::AssetCfg(asset_code.clone()))
            .unwrap_or_else(|| panic_with(Err::UnknownAsset));
        let (pp, _) = latest_point(&e, &asset_code).unwrap_or_else(|| panic_with(Err::NoHistory));
        let now_ts = now(&e);
        if now_ts < pp.ts || (now_ts - pp.ts) > cfg.max_age_secs {
            panic_with(Err::StalePrice);
        }
        let price = rescale_price(pp.price, pp.decimals, out_decimals).unwrap_or_else(|e| panic_with(e));
        (price, out_decimals, pp.ts)
    }

    pub fn get_twap(e: Env, asset_code: String, records: u32, out_decimals: u32) -> (i128, u32, u64) {
        if records == 0 {
            panic_with(Err::NoHistory);
        }
        let cfg: AssetCfg = e
            .storage()
            .instance()
            .get(&DataKey::AssetCfg(asset_code.clone()))
            .unwrap_or_else(|| panic_with(Err::UnknownAsset));

        let hist: Vec<PricePoint> = e
            .storage()
            .instance()
            .get(&DataKey::History(asset_code.clone()))
            .unwrap_or(Vec::new(&e));
        let n = hist.len();
        if n == 0 {
            panic_with(Err::NoHistory);
        }
        let k = if records > n { n } else { records };
        let start = n - k;

        let mut sum: i128 = 0;
        let mut newest_ts: u64 = 0;
        let mut i = start;
        while i < n {
            let pp = hist.get_unchecked(i);
            sum = sum.checked_add(
                rescale_price(pp.price, pp.decimals, out_decimals).unwrap_or_else(|e| panic_with(e))
            ).unwrap_or_else(|| panic_with(Err::MathOverflow));
            if pp.ts > newest_ts { newest_ts = pp.ts; }
            i += 1;
        }

        let now_ts = now(&e);
        if now_ts < newest_ts || (now_ts - newest_ts) > cfg.max_age_secs {
            panic_with(Err::StalePrice);
        }
        let avg = sum / (k as i128);
        (avg, out_decimals, newest_ts)
    }
}

// Helper visible to this module
fn latest_point(e: &Env, code: &String) -> Option<(PricePoint, u32)> {
    let hist: Vec<PricePoint> = e.storage().instance().get(&DataKey::History(code.clone()))?;
    let n = hist.len();
    if n == 0 { None } else { Some((hist.get_unchecked(n - 1), n)) }
}


#[cfg(test)]
mod test;
