#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol,
    Vec,
};
//use soroban_sdk::testutils::{Address as _, Ledger};

const HISTORY_CAP: u32 = 256;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    Feeder,
    PairCfg(String),
    History(String),
}

#[derive(Clone)]
#[contracttype]
pub struct PairCfg {
    pub decimals: u32,
    pub max_age_secs: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct PricePoint {
    pub price: i128,   // scaled by `decimals`
    pub decimals: u32, // redundancy for safety
    pub ts: u64,       // ledger timestamp (seconds)
}

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Err {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotAdmin = 3,
    NotFeeder = 4,
    UnknownPair = 5,
    StalePrice = 6,
    EmptyWindow = 7,
    NoData = 8,
}

/// Rescale an i128 price from `from_dec` to `to_dec`.
fn rescale(_env: &Env, x: i128, from_dec: u32, to_dec: u32) -> i128 {
    if from_dec == to_dec {
        return x;
    }
    let diff = if from_dec > to_dec { from_dec - to_dec } else { to_dec - from_dec };
    let factor = 10i128.pow(diff);
    if from_dec > to_dec {
        x / factor
    } else {
        x.checked_mul(factor).expect("scale overflow")
    }
}
fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| Env::panic_with_error(env, Err::NotInitialized))
}
fn require_admin(env: &Env) {
    let a = read_admin(env);
    a.require_auth();
}
fn read_feeder(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<_, Address>(&DataKey::Feeder)
        .unwrap_or_else(|| Env::panic_with_error(env, Err::NotInitialized))
}
fn require_feeder(env: &Env) {
    let f = read_feeder(env);
    f.require_auth();
}
fn norm_pair(env: &Env, s: String) -> String {
    /// Keep exactly as provided, e.g., "BTC:USDC".
    // (Soroban String is no_std; heavy string ops aren't available.)
    s
}

fn get_cfg(env: &Env, pair: &String) -> PairCfg {
    env.storage()
        .instance()
        .get::<_, PairCfg>(&DataKey::PairCfg(pair.clone()))
        .unwrap_or_else(|| Env::panic_with_error(env, Err::UnknownPair))
}

fn get_history(env: &Env, pair: &String) -> Vec<PricePoint> {
    env.storage()
        .instance()
        .get::<_, Vec<PricePoint>>(&DataKey::History(pair.clone()))
        .unwrap_or(Vec::new(env))
}

fn put_history(env: &Env, pair: &String, hist: &Vec<PricePoint>) {
    env.storage()
        .instance()
        .set(&DataKey::History(pair.clone()), hist);
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    // --- admin & setup ---
    pub fn init(env: Env, admin: Address) {
        if env
            .storage()
            .instance()
            .has(&DataKey::Admin)
        {
            Env::panic_with_error(&env, Err::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn set_feeder(env: Env, feeder: Address) {
        require_admin(&env);
        env.storage().instance().set(&DataKey::Feeder, &feeder);
        env.events()
            .publish((symbol_short!("feeder"),), feeder);
    }

    pub fn upsert_pair(env: Env, pair: String, decimals: u32, max_age_secs: u64) {
        require_admin(&env);
        let p = norm_pair(&env, pair);
        let cfg = PairCfg { decimals, max_age_secs };
        env.storage().instance().set(&DataKey::PairCfg(p.clone()), &cfg);
        env.events().publish(
            (symbol_short!("pair"), p.clone()),
            (decimals, max_age_secs),
        );
    }

    // --- feeding / updates ---
    pub fn poke(env: Env, pair: String, price: i128, decimals: u32, ts: Option<u64>) {
        require_feeder(&env);
        let p = norm_pair(&env, pair);
        let cfg = get_cfg(&env, &p);

        // Allow feeder to override decimals (but warn if mismatched)
        if decimals != cfg.decimals {
            // not fatal; different upstreams sometimes round differently
        }

        let now = env.ledger().timestamp();
        let point = PricePoint {
            price,
            decimals,
            ts: ts.unwrap_or(now),
        };

        // Append & prune
        let mut hist = get_history(&env, &p);
        hist.push_back(point);

        // Trim to HISTORY_CAP (keep most recent)
        let mut trimmed: Vec<PricePoint> = Vec::new(&env);
        let len = hist.len();
        let keep = if len > HISTORY_CAP { HISTORY_CAP } else { len };
        for i in (len - keep)..len {
            trimmed.push_back(hist.get_unchecked(i));
        }
        put_history(&env, &p, &trimmed);

        env.events()
            .publish((symbol_short!("price"), p), (price, decimals, now));
    }

    // --- reads ---
    pub fn get_spot(
        env: Env,
        pair: String,
        override_max_age_secs: Option<u64>,
    ) -> (i128, u32, u64) {
        let p = norm_pair(&env, pair);
        let cfg = get_cfg(&env, &p);
        let max_age = override_max_age_secs.unwrap_or(cfg.max_age_secs);

        let hist = get_history(&env, &p);
        if hist.len() == 0 {
            Env::panic_with_error(&env, Err::NoData);
        }
        let last = hist.get_unchecked(hist.len() - 1);
        let now = env.ledger().timestamp();

        // Prevent unsigned underflow
        let age = now.saturating_sub(last.ts);
        if age > max_age {
            Env::panic_with_error(&env, Err::StalePrice);
        }

        // Always return prices in the pair's configured decimals
        let spot = rescale(&env, last.price, last.decimals, cfg.decimals);
        (spot, cfg.decimals, last.ts)
    }


    pub fn get_twap(
        env: Env,
        pair: String,
        window_secs: u64,
        override_max_age_secs: Option<u64>,
    ) -> (i128, u32, u64, u64) {
        if window_secs == 0 {
            Env::panic_with_error(&env, Err::EmptyWindow);
        }

        let p = norm_pair(&env, pair);
        let cfg = get_cfg(&env, &p);
        let max_age = override_max_age_secs.unwrap_or(cfg.max_age_secs);
        let now = env.ledger().timestamp();
        let start = now.saturating_sub(window_secs);

        let hist = get_history(&env, &p);
        if hist.len() == 0 {
            Env::panic_with_error(&env, Err::NoData);
        }

        // Collect points in [start, now]
        let mut sum: i128 = 0;
        let mut cnt: i128 = 0;
        let mut last_ts: u64 = 0;
        let mut used_decimals: u32 = cfg.decimals;

        for i in 0..hist.len() {
            let pt = hist.get_unchecked(i);
            if pt.ts >= start && pt.ts <= now {
                let adj = rescale(&env, pt.price, pt.decimals, cfg.decimals);
                sum += adj;
                cnt += 1;
                //used_decimals = pt.decimals; // keep latest seen
                if pt.ts > last_ts {
                    last_ts = pt.ts;
                }
            }
        }
        if cnt == 0 {
            // not enough coverage in the window
            Env::panic_with_error(&env, Err::NoData);
        }

        // Naive equal-weight average. (Good enough for MVP; upgrade to time-weighted if needed.)
        let avg = sum / cnt;

        // Staleness guard: last update must not be older than max_age
        if now.saturating_sub(last_ts) > max_age {
            Env::panic_with_error(&env, Err::StalePrice);
        }

        (avg, used_decimals, start, now)
    }
}

mod test;
