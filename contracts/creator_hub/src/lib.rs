#![no_std]
use soroban_sdk::{ contract, contracterror, contractimpl, contracttype, symbol_short, map, Address, Env, Map };

#[derive(Clone)]
#[contracttype]
pub enum DataKey { Admin, Bal(Address) }

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Err { NotInitialized=1, AlreadyInitialized=2, NotAdmin=3 }

fn read_admin(env: &Env) -> Address { env.storage().instance().get::<_, Address>(&DataKey::Admin).unwrap_or_else(|| soroban_sdk::Env::panic_with_error(env, Err::NotInitialized)) }
fn require_admin(env: &Env) { read_admin(env).require_auth(); }

#[contract]
pub struct CreatorHub;

#[contractimpl]
impl CreatorHub {
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) { Env::panic_with_error(&env, Err::AlreadyInitialized); }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Credit fee balances. For now callable by admin. Split proportions example: platform 3, creator 2, stakers 1 of 6 parts total.
    pub fn credit_fees(env: Env, platform: Address, creator: Address, staker_pool: Address, total_fee: i128) {
        require_admin(&env);
        // avoid negative totals
        let tf = if total_fee < 0 { 0 } else { total_fee } as i128;
        let sixth = tf / 6;
        let p = sixth * 3;
        let c = sixth * 2;
        let s = tf - p - c;
        for (addr, amt) in [(platform, p), (creator, c), (staker_pool, s)].iter() {
            let key = DataKey::Bal(addr.clone());
            let cur = env.storage().instance().get::<_, i128>(&key).unwrap_or(0);
            let newv = cur.saturating_add(*amt);
            env.storage().instance().set(&key, &newv);
        }
    }

    pub fn claim(env: Env, who: Address) -> i128 {
        let key = DataKey::Bal(who.clone());
        let cur = env.storage().instance().get::<_, i128>(&key).unwrap_or(0);
        if cur != 0 { env.storage().instance().set(&key, &0i128); }
        cur
    }

    pub fn balance_of(env: Env, who: Address) -> i128 {
        env.storage().instance().get::<_, i128>(&DataKey::Bal(who)).unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
