#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, vec, Address, Env, String
};


#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    OracleId,                  // Contract Address of oracle_adapter
    NextId,
    Round(u64),                // Round data
    Joined(u64, Address),      // anti-double-join marker
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[contracttype]
pub enum RoundStatus {
    Created = 0,
    Locked = 1,
    Settled = 2,
    Canceled = 3,
}

#[derive(Clone)]
#[contracttype]
pub struct Round {
    pub creator: Address,
    pub asset: String,
    pub lock_ts: u64,
    pub settle_ts: u64,
    pub status: RoundStatus,
    pub lock_price: Option<i128>,
    pub lock_decimals: u32,
    pub settle_price: Option<i128>,
    pub settle_decimals: u32,
    pub up_count: u32,
    pub down_count: u32,
}

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Err {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    NotAdmin = 3,
    InvalidTimes = 4,
    RoundNotFound = 5,
    BadState = 6,
    AlreadyJoined = 7,
    JoinClosed = 8,
    OracleNotSet = 9,
}

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| Env::panic_with_error(env, Err::NotInitialized))
}
fn require_admin(env: &Env) {
    read_admin(env).require_auth();
}

fn read_oracle(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<_, Address>(&DataKey::OracleId)
        .unwrap_or_else(|| Env::panic_with_error(env, Err::OracleNotSet))
}

fn get_next_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get::<_, u64>(&DataKey::NextId)
        .unwrap_or(0)
}
fn put_next_id(env: &Env, v: u64) {
    env.storage().instance().set(&DataKey::NextId, &v);
}

fn get_round(env: &Env, id: u64) -> Round {
    env.storage()
        .instance()
        .get::<_, Round>(&DataKey::Round(id))
        .unwrap_or_else(|| Env::panic_with_error(env, Err::RoundNotFound))
}
fn put_round(env: &Env, id: u64, r: &Round) {
    env.storage().instance().set(&DataKey::Round(id), r);
}

#[contract]
pub struct RoundEngine;

#[contractimpl]
impl RoundEngine {
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            Env::panic_with_error(&env, Err::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn set_oracle(env: Env, oracle_id: Address) {
        require_admin(&env);
        env.storage().instance().set(&DataKey::OracleId, &oracle_id);
        env.events().publish((symbol_short!("oracle"),), oracle_id);
    }

    /// Create a round that opens immediately; players can join until `lock_secs` elapses.
    /// After lock, a bot/user calls `lock(id)` to snapshot the price; later `settle(id)` finalizes.
    pub fn create_round(env: Env, creator: Address, asset: String, lock_secs: u64, duration_secs: u64) -> u64 {
        let now = env.ledger().timestamp();
        if lock_secs == 0 || duration_secs == 0 { Env::panic_with_error(&env, Err::InvalidTimes); }
        let lock_ts = now.saturating_add(lock_secs);
        let settle_ts = lock_ts.saturating_add(duration_secs);

        let id = get_next_id(&env);
        put_next_id(&env, id.saturating_add(1));

        let r = Round {
            creator,
            asset,
            lock_ts,
            settle_ts,
            status: RoundStatus::Created,
            lock_price: None,
            lock_decimals: 0,
            settle_price: None,
            settle_decimals: 0,
            up_count: 0,
            down_count: 0,
        };
        put_round(&env, id, &r);
        env.events().publish((symbol_short!("create"), id), (r.lock_ts, r.settle_ts));
        id
    }

    /// side: 0 = Down, 1 = Up
    pub fn join(env: Env, id: u64, who: Address, side: u32) {
        let mut r = get_round(&env, id);
        let now = env.ledger().timestamp();
        if now >= r.lock_ts { Env::panic_with_error(&env, Err::JoinClosed); }
        if env.storage().instance().has(&DataKey::Joined(id, who.clone())) {
            Env::panic_with_error(&env, Err::AlreadyJoined);
        }
        // record join
        env.storage().instance().set(&DataKey::Joined(id, who.clone()), &true);
        if side == 1 { r.up_count = r.up_count.saturating_add(1); } else { r.down_count = r.down_count.saturating_add(1); }
        put_round(&env, id, &r);
        env.events().publish((symbol_short!("join"), id), (who, side));
    }

    /// Snapshot the lock price via oracle; callable at/after lock_ts.
    pub fn lock(env: Env, id: u64) {
        let mut r = get_round(&env, id);
        if r.status != RoundStatus::Created { Env::panic_with_error(&env, Err::BadState); }
        let now = env.ledger().timestamp();
        if now < r.lock_ts { Env::panic_with_error(&env, Err::BadState); }
        let oracle = read_oracle(&env);
        // (price, decimals, ts)
        let (p,d,_): (i128,u32,u64) = env.invoke_contract(&oracle, &symbol_short!("get_spot"), vec![&env, r.asset.clone().into_val(&env), Option::<u64>::None.into_val(&env)]);
        r.lock_price = Some(p);
        r.lock_decimals = d;
        r.status = RoundStatus::Locked;
        put_round(&env, id, &r);
        env.events().publish((symbol_short!("lock"), id), (p, d));
    }

    /// Finalize the round, capturing settle price and marking winner side in events.
    pub fn settle(env: Env, id: u64) {
        let mut r = get_round(&env, id);
        if r.status != RoundStatus::Locked { Env::panic_with_error(&env, Err::BadState); }
        let now = env.ledger().timestamp();
        if now < r.settle_ts { Env::panic_with_error(&env, Err::BadState); }
        let oracle = read_oracle(&env);
        let (p, d, _ts): (i128, u32, u64) = env.invoke_contract(&oracle, &symbol_short!("get_spot"), vec![&env, r.pair.clone().into_val(&env), Option::<u64>::None.into_val(&env)]);
        r.settle_price = Some(p);
        r.settle_decimals = d;
        r.status = RoundStatus::Settled;
        put_round(&env, id, &r);

        let mut winner: u32 = 2; // 0=down,1=up,2=tie
        if let (Some(lp), Some(sp)) = (r.lock_price, r.settle_price) {
            if sp > lp { winner = 1; } else if sp < lp { winner = 0; } else { winner = 2; }
        }
        env.events().publish((symbol_short!("settle"), id), (p, d, winner));
    }

    pub fn cancel(env: Env, id: u64) {
        require_admin(&env);
        let mut r = get_round(&env, id);
        if r.status == RoundStatus::Settled || r.status == RoundStatus::Canceled {
            Env::panic_with_error(&env, Err::BadState);
        }
        r.status = RoundStatus::Canceled;
        put_round(&env, id, &r);
        env.events().publish((symbol_short!("cancel"), id), ());
    }

    pub fn get_round(env: Env, id: u64) -> Round { get_round(&env, id) }
}

// Bring IntoVal for invoke_contract args
use soroban_sdk::IntoVal;

#[cfg(test)]
mod test;
