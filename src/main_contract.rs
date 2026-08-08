/**
 * © جميع الحقوق محفوظة 2026 - المطور: فؤاد يحيى عزمان
 * البريد الإلكتروني: fuad.mithaq@gmail.com
 * Pi Chat: @Fuad207
 * مشروع: ميثاق (Mithaq) - العقد الذكي الأسطوري (v8.0 - Hybrid Liquidity Engine)
 * الملف: contracts/src/main_contract.rs
 * الوصف: دمج محرك السيولة الهجين (HLE) مع نظام الشرائح المتدرجة لقروض السمعة.
 * تحديث: توزيع المصادرات 50/40/10 (منصة/صندوق أمان/خزينة DAO)
 * Developed & Architected by: Fuad Azman
 */

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, String, Symbol, BytesN, Bytes, Map, Vec,
    token::{TokenClient},
    Val, TryIntoVal,
};

use crate::certificate::CertificateRegistryClient;
use crate::azman_token::AzmanTokenClient;

// ===== 1. أنواع البيانات =====
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    ContractCounter,
    DisputeCounter,
    Commitment(u64),
    Dispute(u64),
    Reputation(Address),
    Admin,
    PiServer,
    AzmanToken,
    CertificateRegistry,
    DeliveryProof(u64),
    ProposalFeeShare,
    LiquidityPool,
    FeeRecyclePool,
    LiquidityMetrics,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    pub id: u64,
    pub creator: Address,
    pub counterparty: Address,
    pub original_value: i128,
    pub net_value: i128,
    pub down_payment: i128,
    pub first_release_amount: i128,
    pub second_release_amount: i128,
    pub contract_type: Symbol,
    pub status: Symbol,
    pub deadline: u64,
    pub accepted_at: u64,
    pub review_deadline: u64,
    pub auto_release_deadline: u64,
    pub first_release_done: bool,
    pub payment_status: Symbol,
    pub custom_step: u32,
    pub escrow_balance: i128,
    pub platform_fee_percent: i128,
    pub insurance_fee_percent: i128,
    pub legal_doc_hash: String,
    pub extra_data: Map<String, Val>,
    pub created_at: u64,
    pub contributes_to_pool: bool,
    pub liquidity_contribution: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    pub id: u64,
    pub contract_id: u64,
    pub plaintiff: Address,
    pub defendant: Address,
    pub status: Symbol,
    pub penalty: Symbol,
    pub opened_at: u64,
    pub resolved_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityPoolState {
    pub total_pooled: i128,
    pub total_recycled_fees: i128,
    pub active_contributors: u32,
    pub pool_utilization: i128,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentParams {
    pub original_value: i128,
    pub net_value: i128,
    pub down_payment: i128,
    pub contract_type: Symbol,
    pub deadline: u64,
    pub pi_payment_id: String,
    pub first_release_amount: i128,
    pub second_release_amount: i128,
    pub legal_doc_hash: String,
}

// ===== 2. الأحداث =====
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractCreated { pub id: u64, pub creator: Address, pub contract_type: Symbol, pub net_value: i128, pub down_payment: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractAccepted { pub id: u64, pub counterparty: Address, pub escrowed_amount: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractCancelled { pub id: u64, pub by: Address, pub refunded: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentUpdated { pub id: u64, pub status: Symbol }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryConfirmed { pub id: u64, pub amount: i128, pub proof_hash: String }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractCompleted { pub id: u64, pub final_payout: i128, pub fees_paid: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoReleased { pub id: u64, pub payout: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeOpened { pub dispute_id: u64, pub contract_id: u64, pub by: Address, pub reason: String }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisputeResolved { pub dispute_id: u64, pub winner: Address, pub verdict: String, pub payout: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneReleased { pub id: u64, pub milestone: u32, pub amount: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallmentPaid { pub id: u64, pub installment: u32, pub amount: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalRejected { pub proposal_id: String, pub developer: Address, pub fund_share: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractUpgraded { pub admin: Address, pub new_wasm_hash: BytesN<32> }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityContributed { pub contract_id: u64, pub amount: i128, pub total_pooled: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityWithdrawn { pub contract_id: u64, pub amount: i128, pub total_pooled: i128 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeesRecycled { pub amount: i128, pub recycle_end: u64 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolRebalanced { pub old_utilization: i128, pub new_utilization: i128, pub fee_adjustment: i128 }

// ===== 3. الثوابت =====
const REVIEW_PERIOD_SECONDS: u64 = 259_200;
const AUTO_RELEASE_SECONDS: u64 = 172_800;
const REPUTATION_FRAUD_PENALTY: i128 = -10;
const REPUTATION_BREACH_PENALTY: i128 = -5;
const REPUTATION_NEGLECT_PENALTY: i128 = -2;
const REPUTATION_COMPLETION_REWARD: i128 = 2;
const LEDGERS_IN_30_DAYS: u32 = 518_400;

const TIER_DIAMOND_REP: i128 = 95;
const TIER_DIAMOND_LTV: i128 = 95;
const TIER_GOLD_REP: i128 = 90;
const TIER_GOLD_LTV: i128 = 75;
const TIER_SILVER_REP: i128 = 80;
const TIER_SILVER_LTV: i128 = 50;

const POOL_CONTRIBUTION_RATIO: i128 = 30;
const FEE_RECYCLE_DURATION: u64 = 604_800;
const MIN_POOL_UTILIZATION: i128 = 20;
const MAX_POOL_UTILIZATION: i128 = 80;
const LIQUIDITY_SURCHARGE: i128 = 5;
const LIQUIDITY_DISCOUNT: i128 = 5;

const FORFEITURE_PLATFORM_SHARE: i128 = 50;
const FORFEITURE_SAFETY_SHARE: i128 = 40;
const FORFEITURE_DAO_SHARE: i128 = 10;

const S_PENDING: Symbol = symbol_short!("PEND");
const S_ACTIVE: Symbol = symbol_short!("ACTV");
const S_CANCEL: Symbol = symbol_short!("CANC");
const S_AWAIT: Symbol = symbol_short!("WAIT");
const S_COMPLET: Symbol = symbol_short!("COMP");
const S_AUTO: Symbol = symbol_short!("AUTO");
const S_DISPUTE: Symbol = symbol_short!("DISP");
const S_ARBITR: Symbol = symbol_short!("ARBT");
const S_OPEN: Symbol = symbol_short!("OPEN");

const P_PENDING: Symbol = symbol_short!("PEND");
const P_COMPLET: Symbol = symbol_short!("COMP");
const P_CANCEL: Symbol = symbol_short!("CANC");

const C_CONSTRUCT: Symbol = symbol_short!("CONS");
const C_TUITION: Symbol = symbol_short!("TUIT");
const C_REP_LEND: Symbol = symbol_short!("REPL");

const PEN_FRAUD: Symbol = symbol_short!("FRAU");
const PEN_BREACH: Symbol = symbol_short!("BREA");
const PEN_NEGLECT: Symbol = symbol_short!("NEGL");

#[contract]
pub struct MithaqContract;

#[contractimpl]
impl MithaqContract {

    fn get_reputation_internal(env: &Env, user: &Address) -> i128 {
        let key = DataKey::Reputation(user.clone());
        if let Some(rep) = env.storage().persistent().get(&key) {
            env.storage().persistent().extend_ttl(&key, LEDGERS_IN_30_DAYS, LEDGERS_IN_30_DAYS);
            rep
        } else {
            0i128
        }
    }

    fn add_reputation(env: &Env, user: &Address, points: i128) {
        let current = Self::get_reputation_internal(env, user);
        let new_rep = (current + points).max(0);
        let key = DataKey::Reputation(user.clone());
        env.storage().persistent().set(&key, &new_rep);
        env.storage().persistent().extend_ttl(&key, LEDGERS_IN_30_DAYS, LEDGERS_IN_30_DAYS);
    }

    fn get_reputation_tier_limit(rep: i128) -> i128 {
        if rep >= TIER_DIAMOND_REP { TIER_DIAMOND_LTV }
        else if rep >= TIER_GOLD_REP { TIER_GOLD_LTV }
        else if rep >= TIER_SILVER_REP { TIER_SILVER_LTV }
        else { 0 }
    }

    fn calculate_dynamic_fees(env: &Env, creator: &Address) -> (i128, i128) {
        let rep = Self::get_reputation_internal(env, creator);
        let mut platform_fee = 2i128;
        let insurance_fee = 1i128;
        if rep >= 100 { platform_fee = 1; }
        else if rep < 20 { platform_fee = 4; }

        let pool_state = Self::get_liquidity_pool_state(env);
        if pool_state.pool_utilization < MIN_POOL_UTILIZATION {
            platform_fee += LIQUIDITY_SURCHARGE;
        } else if pool_state.pool_utilization > MAX_POOL_UTILIZATION {
            platform_fee = (platform_fee - LIQUIDITY_DISCOUNT).max(0);
        }

        (platform_fee, insurance_fee)
    }

    fn get_commitment(env: &Env, id: u64) -> Commitment {
        let key = DataKey::Commitment(id);
        let c: Commitment = env.storage().persistent().get(&key).expect("Commitment not found");
        env.storage().persistent().extend_ttl(&key, LEDGERS_IN_30_DAYS, LEDGERS_IN_30_DAYS);
        c
    }

    fn save_commitment(env: &Env, id: u64, c: &Commitment) {
        let key = DataKey::Commitment(id);
        env.storage().persistent().set(&key, c);
        env.storage().persistent().extend_ttl(&key, LEDGERS_IN_30_DAYS, LEDGERS_IN_30_DAYS);
    }

    fn transfer_tokens(env: &Env, token_addr: &Address, to: &Address, amount: i128) {
        if amount <= 0 { return; }
        let client = TokenClient::new(env, token_addr);
        client.transfer(&env.current_contract_address(), to, &amount);
    }

    // ==================== محرك السيولة الهجين ====================

    fn get_liquidity_pool_state(env: &Env) -> LiquidityPoolState {
        let key = DataKey::LiquidityMetrics;
        let state: LiquidityPoolState = env.storage().persistent().get(&key).unwrap_or(LiquidityPoolState {
            total_pooled: 0,
            total_recycled_fees: 0,
            active_contributors: 0,
            pool_utilization: 50,
            last_updated: env.ledger().timestamp(),
        });
        env.storage().persistent().extend_ttl(&key, LEDGERS_IN_30_DAYS, LEDGERS_IN_30_DAYS);
        state
    }

    fn save_liquidity_pool_state(env: &Env, state: &LiquidityPoolState) {
        let key = DataKey::LiquidityMetrics;
        env.storage().persistent().set(&key, state);
        env.storage().persistent().extend_ttl(&key, LEDGERS_IN_30_DAYS, LEDGERS_IN_30_DAYS);
    }

    fn contribute_to_pool(env: &Env, commitment_id: u64, escrow_amount: i128) -> i128 {
        let contribution = (escrow_amount * POOL_CONTRIBUTION_RATIO) / 100;
        if contribution <= 0 { return 0; }

        let mut pool = Self::get_liquidity_pool_state(env);
        pool.total_pooled += contribution;
        pool.active_contributors += 1;

        let total_escrow = Self::get_total_escrow_balance(env);
        if total_escrow > 0 {
            pool.pool_utilization = (pool.total_pooled * 100) / total_escrow;
        }
        pool.last_updated = env.ledger().timestamp();

        Self::save_liquidity_pool_state(env, &pool);

        env.events().publish(
            (Symbol::new(env, "liquidity_contributed"),),
            LiquidityContributed {
                contract_id: commitment_id,
                amount: contribution,
                total_pooled: pool.total_pooled,
            },
        );

        contribution
    }

    fn withdraw_from_pool(env: &Env, commitment_id: u64, contributed_amount: i128) {
        if contributed_amount <= 0 { return; }

        let mut pool = Self::get_liquidity_pool_state(env);
        pool.total_pooled = (pool.total_pooled - contributed_amount).max(0);
        pool.active_contributors = pool.active_contributors.saturating_sub(1);

        let total_escrow = Self::get_total_escrow_balance(env);
        if total_escrow > 0 {
            pool.pool_utilization = (pool.total_pooled * 100) / total_escrow;
        } else {
            pool.pool_utilization = 50;
        }
        pool.last_updated = env.ledger().timestamp();

        Self::save_liquidity_pool_state(env, &pool);

        env.events().publish(
            (Symbol::new(env, "liquidity_withdrawn"),),
            LiquidityWithdrawn {
                contract_id: commitment_id,
                amount: contributed_amount,
                total_pooled: pool.total_pooled,
            },
        );
    }

    fn recycle_fees(env: &Env, fee_amount: i128) {
        if fee_amount <= 0 { return; }

        let mut pool = Self::get_liquidity_pool_state(env);
        pool.total_recycled_fees += fee_amount;
        pool.total_pooled += fee_amount;

        let total_escrow = Self::get_total_escrow_balance(env);
        if total_escrow > 0 {
            pool.pool_utilization = (pool.total_pooled * 100) / total_escrow;
        }

        let recycle_end = env.ledger().timestamp() + FEE_RECYCLE_DURATION;
        let recycle_key = DataKey::FeeRecyclePool;
        env.storage().persistent().set(&recycle_key, &(fee_amount, recycle_end));
        env.storage().persistent().extend_ttl(&recycle_key, LEDGERS_IN_30_DAYS, LEDGERS_IN_30_DAYS);

        pool.last_updated = env.ledger().timestamp();
        Self::save_liquidity_pool_state(env, &pool);

        env.events().publish(
            (Symbol::new(env, "fees_recycled"),),
            FeesRecycled { amount: fee_amount, recycle_end },
        );
    }

    fn release_recycled_fees(env: &Env) -> i128 {
        let recycle_key = DataKey::FeeRecyclePool;
        if let Some((amount, recycle_end)) = env.storage().persistent().get::<_, (i128, u64)>(&recycle_key) {
            if env.ledger().timestamp() >= recycle_end {
                let mut pool = Self::get_liquidity_pool_state(env);
                pool.total_recycled_fees = (pool.total_recycled_fees - amount).max(0);
                pool.total_pooled = (pool.total_pooled - amount).max(0);

                let total_escrow = Self::get_total_escrow_balance(env);
                if total_escrow > 0 {
                    pool.pool_utilization = (pool.total_pooled * 100) / total_escrow;
                } else {
                    pool.pool_utilization = 50;
                }
                pool.last_updated = env.ledger().timestamp();
                Self::save_liquidity_pool_state(env, &pool);

                env.storage().persistent().remove(&recycle_key);
                return amount;
            }
        }
        0
    }

    fn get_total_escrow_balance(env: &Env) -> i128 {
        let counter = env.storage().persistent().get::<_, u64>(&DataKey::ContractCounter).unwrap_or(0);
        let mut total: i128 = 0;
        for id in 1..=counter {
            let key = DataKey::Commitment(id);
            if let Some(c) = env.storage().persistent().get::<_, Commitment>(&key) {
                if c.status == S_ACTIVE || c.status == S_AWAIT {
                    total += c.escrow_balance;
                }
            }
        }
        total
    }

    fn process_hybrid_liquidity(env: &Env, commitment: &Commitment, fees_paid: i128) {
        Self::release_recycled_fees(env);

        if fees_paid > 0 {
            let recycle_amount = fees_paid / 2;
            Self::recycle_fees(env, recycle_amount);
        }

        if commitment.contributes_to_pool && commitment.liquidity_contribution > 0 {
            Self::withdraw_from_pool(env, commitment.id, commitment.liquidity_contribution);
        }

        let pool = Self::get_liquidity_pool_state(env);
        let old_utilization = pool.pool_utilization;
        let new_utilization = if pool.total_pooled > 0 {
            let total_escrow = Self::get_total_escrow_balance(env);
            if total_escrow > 0 {
                (pool.total_pooled * 100) / total_escrow
            } else {
                50
            }
        } else {
            50
        };

        if old_utilization != new_utilization {
            let mut updated_pool = pool;
            updated_pool.pool_utilization = new_utilization;
            updated_pool.last_updated = env.ledger().timestamp();
            Self::save_liquidity_pool_state(env, &updated_pool);

            let fee_adjustment = if new_utilization < MIN_POOL_UTILIZATION {
                LIQUIDITY_SURCHARGE
            } else if new_utilization > MAX_POOL_UTILIZATION {
                -LIQUIDITY_DISCOUNT
            } else {
                0
            };

            env.events().publish(
                (Symbol::new(env, "pool_rebalanced"),),
                PoolRebalanced { old_utilization, new_utilization, fee_adjustment },
            );
        }
    }

    // ==================== 1. التهيئة ====================
    pub fn initialize(env: Env, admin: Address, pi_server: Address, azman_token: Address, cert_registry: Address) {
        if env.storage().persistent().has(&DataKey::Admin) { panic!("Already initialized"); }
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::PiServer, &pi_server);
        env.storage().persistent().set(&DataKey::AzmanToken, &azman_token);
        env.storage().persistent().set(&DataKey::CertificateRegistry, &cert_registry);
        env.storage().persistent().set(&DataKey::ContractCounter, &0u64);
        env.storage().persistent().set(&DataKey::DisputeCounter, &0u64);
        env.storage().persistent().set(&DataKey::ProposalFeeShare, &50_000_000i128);

        let initial_pool = LiquidityPoolState {
            total_pooled: 0,
            total_recycled_fees: 0,
            active_contributors: 0,
            pool_utilization: 50,
            last_updated: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&DataKey::LiquidityMetrics, &initial_pool);
    }

    // ==================== 2. إنشاء العقد ====================
    pub fn create_commitment(
        env: Env,
        creator: Address,
        counterparty: Address,
        params: CommitmentParams,
    ) -> u64 {
        env.storage().persistent().get::<_, Address>(&DataKey::PiServer).expect("PiServer not set").require_auth();

        if params.net_value <= 0 { panic!("Net value must be > 0"); }
        if params.down_payment < 0 || params.down_payment > params.net_value { panic!("Invalid down payment amount"); }
        if creator == counterparty { panic!("Self-dealing is strictly prohibited"); }
        if params.first_release_amount + params.second_release_amount != params.net_value { panic!("Amounts mismatch"); }
        if params.deadline <= env.ledger().timestamp() { panic!("Deadline must be in future"); }

        if params.contract_type == C_REP_LEND {
            let reputation = Self::get_reputation_internal(&env, &creator);
            let max_ltv = Self::get_reputation_tier_limit(reputation);
            if max_ltv == 0 { panic!("Credit Rejected: Reputation below 80 (Silver Tier)"); }
            let loan_amount = params.net_value - params.down_payment;
            let requested_ltv = (loan_amount * 100) / params.net_value;
            if requested_ltv > max_ltv {
                panic!("Credit Rejected: Requested LTV exceeds allowed tier limit");
            }
        } else {
            if params.down_payment != params.net_value {
                panic!("Standard contracts require 100% down payment (Zero LTV)");
            }
        }

        let (dyn_platform_fee, dyn_insurance_fee) = Self::calculate_dynamic_fees(&env, &creator);
        let id = env.storage().persistent().get::<_, u64>(&DataKey::ContractCounter).unwrap_or(0) + 1;
        env.storage().persistent().set(&DataKey::ContractCounter, &id);

        let commitment = Commitment {
            id,
            creator: creator.clone(),
            counterparty: counterparty.clone(),
            original_value: params.original_value,
            net_value: params.net_value,
            down_payment: params.down_payment,
            first_release_amount: params.first_release_amount,
            second_release_amount: params.second_release_amount,
            contract_type: params.contract_type.clone(),
            status: S_PENDING,
            deadline: params.deadline,
            accepted_at: 0, review_deadline: 0, auto_release_deadline: 0,
            first_release_done: false, payment_status: P_PENDING, custom_step: 0, escrow_balance: 0,
            platform_fee_percent: dyn_platform_fee, insurance_fee_percent: dyn_insurance_fee,
            legal_doc_hash: params.legal_doc_hash.clone(),
            extra_data: Map::new(&env),
            created_at: env.ledger().timestamp(),
            contributes_to_pool: false,
            liquidity_contribution: 0,
        };

        Self::save_commitment(&env, id, &commitment);
        
        env.events().publish(
            (Symbol::new(&env, "contract_created"),),
            ContractCreated { id, creator, contract_type: params.contract_type, net_value: params.net_value, down_payment: params.down_payment },
        );

        id
    }

    // ==================== 3. القبول وحجز الرصيد ====================
    pub fn accept_commitment(env: Env, id: u64, funder: Address, verified_doc_hash: String) {
        funder.require_auth();
        let mut c = Self::get_commitment(&env, id);

        if c.status != S_PENDING { panic!("Contract is not pending"); }
        if env.ledger().timestamp() > c.deadline { panic!("Contract offer expired"); }
        if c.legal_doc_hash != verified_doc_hash { panic!("Legal Hash Mismatch"); }
        if funder != c.counterparty { panic!("Only counterparty can accept"); }

        let azman_addr = env.storage().persistent().get::<_, Address>(&DataKey::AzmanToken).expect("Token not configured");
        let token_client = TokenClient::new(&env, &azman_addr);
        token_client.transfer(&funder, &env.current_contract_address(), &c.down_payment);

        c.escrow_balance = c.down_payment;
        c.status = S_ACTIVE;
        c.accepted_at = env.ledger().timestamp();

        let contribution = Self::contribute_to_pool(&env, id, c.down_payment);
        if contribution > 0 {
            c.contributes_to_pool = true;
            c.liquidity_contribution = contribution;
        }

        Self::save_commitment(&env, id, &c);

        env.events().publish(
            (Symbol::new(&env, "contract_accepted"),),
            ContractAccepted { id, counterparty: c.counterparty, escrowed_amount: c.down_payment },
        );
    }

    // ==================== 4. الإلغاء القانوني ====================
    pub fn cancel_commitment(env: Env, id: u64, caller: Address, reason: Symbol) {
        caller.require_auth();
        let mut c = Self::get_commitment(&env, id);

        let c_force_maj = symbol_short!("FORCE_MAJ");
        let c_mutual = symbol_short!("MUTUAL");

        if c.status != S_PENDING && reason != c_mutual && reason != c_force_maj {
            panic!("Active contracts require mutual consent or force majeure to cancel");
        }

        if caller != c.creator && caller != c.counterparty { panic!("Not a party"); }

        c.status = S_CANCEL;
        let refunded = c.escrow_balance;

        if c.contributes_to_pool {
            Self::withdraw_from_pool(&env, id, c.liquidity_contribution);
            c.contributes_to_pool = false;
            c.liquidity_contribution = 0;
        }

        if refunded > 0 {
            let azman_addr = env.storage().persistent().get::<_, Address>(&DataKey::AzmanToken).unwrap();
            Self::transfer_tokens(&env, &azman_addr, &c.counterparty, refunded);
            c.escrow_balance = 0;
        }

        Self::save_commitment(&env, id, &c);

        env.events().publish(
            (Symbol::new(&env, "contract_cancelled"),),
            ContractCancelled { id, by: caller, refunded },
        );
    }

    // ==================== 5. تحديث حالة الدفع ====================
    pub fn update_payment_status(env: Env, id: u64, payment_status: Symbol) {
        env.storage().persistent().get::<_, Address>(&DataKey::PiServer).expect("PiServer not set").require_auth();
        let mut c = Self::get_commitment(&env, id);
        c.payment_status = payment_status.clone();
        if payment_status == P_CANCEL {
            c.status = S_CANCEL;
        }
        Self::save_commitment(&env, id, &c);

        env.events().publish(
            (Symbol::new(&env, "payment_updated"),),
            PaymentUpdated { id, status: payment_status },
        );
    }

    // ==================== 6. تأكيد التسليم ====================
    pub fn confirm_delivery(env: Env, id: u64, counterparty: Address, biometric_hash: String) {
        counterparty.require_auth();
        let mut c = Self::get_commitment(&env, id);

        if counterparty != c.counterparty { panic!("Not counterparty"); }
        if c.status != S_ACTIVE { panic!("Not active"); }
        if c.first_release_done { panic!("Already released"); }

        c.first_release_done = true;
        c.status = S_AWAIT;
        c.review_deadline = env.ledger().timestamp() + REVIEW_PERIOD_SECONDS;
        c.auto_release_deadline = c.review_deadline + AUTO_RELEASE_SECONDS;
        c.extra_data.set(String::from_str(&env, "delivery_proof_hash"), biometric_hash.to_val());

        Self::save_commitment(&env, id, &c);

        env.events().publish(
            (Symbol::new(&env, "delivery_confirmed"),),
            DeliveryConfirmed { id, amount: c.first_release_amount, proof_hash: biometric_hash },
        );
    }

    // ==================== 7. تأكيد المراجعة ====================
    pub fn confirm_review(env: Env, id: u64, counterparty: Address) {
        counterparty.require_auth();
        let mut c = Self::get_commitment(&env, id);
        if counterparty != c.counterparty { panic!("Not counterparty"); }
        if c.status != S_AWAIT { panic!("Not in review"); }

        let payout = c.escrow_balance;
        c.escrow_balance = 0;
        c.status = S_COMPLET;
        Self::add_reputation(&env, &c.creator, REPUTATION_COMPLETION_REWARD);
        Self::add_reputation(&env, &c.counterparty, REPUTATION_COMPLETION_REWARD);
        Self::save_commitment(&env, id, &c);

        let id_bytes = Bytes::from_slice(&env, &c.id.to_be_bytes());
        let cert_id = env.crypto().sha256(&id_bytes);
        let cert_registry_addr = env.storage().persistent().get::<_, Address>(&DataKey::CertificateRegistry).unwrap();

        env.authorize_as_current_contract(Vec::new(&env));

        let cert_id_bytes: BytesN<32> = cert_id.to_bytes();
        let imprint_str = String::from_str(&env, "Mithaq Verified");
        CertificateRegistryClient::new(&env, &cert_registry_addr).issue_certificate(
            &env.current_contract_address(),
            &cert_id_bytes,
            &c.creator,
            &c.counterparty,
            &imprint_str,
        );

        let azman_addr = env.storage().persistent().get::<_, Address>(&DataKey::AzmanToken).unwrap();
        Self::transfer_tokens(&env, &azman_addr, &c.creator, payout);

        let fees_paid = (payout * c.platform_fee_percent) / 100;
        AzmanTokenClient::new(&env, &azman_addr).process_contract_fees(
            &env.current_contract_address(), &c.creator, &payout,
        );

        Self::process_hybrid_liquidity(&env, &c, fees_paid);

        env.events().publish(
            (Symbol::new(&env, "contract_completed"),),
            ContractCompleted { id, final_payout: payout, fees_paid },
        );
    }

    // ==================== 8. تحرير تلقائي ====================
    pub fn auto_release(env: Env, id: u64) {
        let mut c = Self::get_commitment(&env, id);

        if c.status != S_AWAIT { panic!("Not in review"); }
        if env.ledger().timestamp() < c.auto_release_deadline { panic!("Grace period active"); }

        let payout = c.escrow_balance;
        c.escrow_balance = 0;
        c.status = S_AUTO;
        Self::add_reputation(&env, &c.counterparty, REPUTATION_NEGLECT_PENALTY);
        Self::add_reputation(&env, &c.creator, REPUTATION_COMPLETION_REWARD);

        let azman_addr = env.storage().persistent().get::<_, Address>(&DataKey::AzmanToken).unwrap();
        Self::transfer_tokens(&env, &azman_addr, &c.creator, payout);

        let fees_paid = (payout * c.platform_fee_percent) / 100;
        AzmanTokenClient::new(&env, &azman_addr).process_contract_fees(
            &env.current_contract_address(), &c.creator, &payout,
        );

        Self::process_hybrid_liquidity(&env, &c, fees_paid);

        Self::save_commitment(&env, id, &c);

        env.events().publish(
            (Symbol::new(&env, "auto_released"),),
            AutoReleased { id, payout },
        );
    }

    // ==================== 9. فتح نزاع ====================
    pub fn open_dispute(env: Env, id: u64, caller: Address, reason: String) -> u64 {
        caller.require_auth();
        let mut c = Self::get_commitment(&env, id);

        if caller != c.creator && caller != c.counterparty { panic!("Locus standi required"); }
        if c.status != S_AWAIT && c.status != S_ACTIVE { panic!("Ineligible state for dispute"); }

        c.status = S_DISPUTE;
        Self::save_commitment(&env, id, &c);

        let dispute_id = env.storage().persistent().get::<_, u64>(&DataKey::DisputeCounter).unwrap_or(0) + 1;
        env.storage().persistent().set(&DataKey::DisputeCounter, &dispute_id);

        let dispute = Dispute {
            id: dispute_id,
            contract_id: id,
            plaintiff: caller.clone(),
            defendant: if caller == c.creator { c.counterparty.clone() } else { c.creator.clone() },
            status: S_OPEN,
            penalty: symbol_short!("PENDING"),
            opened_at: env.ledger().timestamp(),
            resolved_at: 0,
        };
        env.storage().persistent().set(&DataKey::Dispute(dispute_id), &dispute);

        env.events().publish(
            (Symbol::new(&env, "dispute_opened"),),
            DisputeOpened { dispute_id, contract_id: id, by: caller, reason },
        );

        dispute_id
    }

    // ==================== 10. حل النزاع ====================
    pub fn resolve_dispute(env: Env, dispute_id: u64, winner: Address, verdict_text: String, penalty_type: Symbol) {
        env.storage().persistent().get::<_, Address>(&DataKey::Admin).expect("Admin not set").require_auth();
        let mut d: Dispute = env.storage().persistent().get(&DataKey::Dispute(dispute_id)).expect("Dispute not found");
        if d.status != S_OPEN { panic!("Case closed"); }

        let mut c = Self::get_commitment(&env, d.contract_id);

        let penalty_points = if penalty_type == PEN_FRAUD {
            REPUTATION_FRAUD_PENALTY
        } else if penalty_type == PEN_BREACH {
            REPUTATION_BREACH_PENALTY
        } else {
            REPUTATION_NEGLECT_PENALTY
        };

        let loser = if winner == c.creator { c.counterparty.clone() } else { c.creator.clone() };
        Self::add_reputation(&env, &loser, penalty_points);
        Self::add_reputation(&env, &winner, REPUTATION_COMPLETION_REWARD);

        d.status = S_ARBITR;
        d.penalty = penalty_type.clone();
        d.resolved_at = env.ledger().timestamp();
        c.status = S_COMPLET;

        let payout = c.escrow_balance;
        c.escrow_balance = 0;

        let azman_addr = env.storage().persistent().get::<_, Address>(&DataKey::AzmanToken).unwrap();
        Self::transfer_tokens(&env, &azman_addr, &winner, payout);

        let fees_paid = (payout * c.platform_fee_percent) / 100;
        Self::process_hybrid_liquidity(&env, &c, fees_paid);

        env.storage().persistent().set(&DataKey::Dispute(dispute_id), &d);
        Self::save_commitment(&env, d.contract_id, &c);

        env.events().publish(
            (Symbol::new(&env, "dispute_resolved"),),
            DisputeResolved { dispute_id, winner, verdict: verdict_text, payout },
        );
    }

    // ==================== العقود المتخصصة ====================

    pub fn release_milestone(env: Env, id: u64, caller: Address, milestone: u32) {
        caller.require_auth();
        let mut c = Self::get_commitment(&env, id);

        if c.contract_type != C_CONSTRUCT { panic!("Strict Type: Not construction"); }
        if caller != c.counterparty { panic!("Only counterparty authorization"); }
        if c.status != S_ACTIVE && c.status != S_AWAIT { panic!("Contract frozen or complete"); }

        let current_milestone: u32 = c.extra_data
            .get(String::from_str(&env, "milestone"))
            .unwrap_or(Val::from_u32(0).into())
            .try_into_val(&env)
            .unwrap_or(0u32);

        if milestone <= current_milestone { panic!("Double spending/release attempt"); }

        c.extra_data.set(String::from_str(&env, "milestone"), Val::from_u32(milestone).into());

        let amount_per_milestone = c.down_payment / 10;
        let azman_addr = env.storage().persistent().get::<_, Address>(&DataKey::AzmanToken).unwrap();
        Self::transfer_tokens(&env, &azman_addr, &c.creator, amount_per_milestone);
        c.escrow_balance -= amount_per_milestone;

        if milestone >= 10 {
            c.status = S_COMPLET;
            Self::add_reputation(&env, &c.creator, REPUTATION_COMPLETION_REWARD);
            Self::add_reputation(&env, &c.counterparty, REPUTATION_COMPLETION_REWARD);
            let fees_paid = (c.down_payment * c.platform_fee_percent) / 100;
            Self::process_hybrid_liquidity(&env, &c, fees_paid);
        }

        Self::save_commitment(&env, id, &c);

        env.events().publish(
            (Symbol::new(&env, "milestone_released"),),
            MilestoneReleased { id, milestone, amount: amount_per_milestone },
        );
    }

    pub fn pay_installment(env: Env, id: u64, caller: Address, installment_num: u32) {
        caller.require_auth();
        let mut c = Self::get_commitment(&env, id);

        if c.contract_type != C_TUITION { panic!("Strict Type: Not tuition"); }
        if caller != c.counterparty { panic!("Unauthorized payer"); }
        if c.status != S_ACTIVE { panic!("Account not in good standing"); }

        let current_installment: u32 = c.extra_data
            .get(String::from_str(&env, "installment"))
            .unwrap_or(Val::from_u32(0).into())
            .try_into_val(&env)
            .unwrap_or(0u32);

        if installment_num <= current_installment { panic!("Installment previously cleared"); }

        c.extra_data.set(String::from_str(&env, "installment"), Val::from_u32(installment_num).into());

        let amount_per_installment = c.down_payment / 9;

        if installment_num >= 9 {
            c.status = S_COMPLET;
            Self::add_reputation(&env, &c.creator, REPUTATION_COMPLETION_REWARD);
            Self::add_reputation(&env, &c.counterparty, REPUTATION_COMPLETION_REWARD);
            let fees_paid = (c.down_payment * c.platform_fee_percent) / 100;
            Self::process_hybrid_liquidity(&env, &c, fees_paid);
        }

        Self::save_commitment(&env, id, &c);

        env.events().publish(
            (Symbol::new(&env, "installment_paid"),),
            InstallmentPaid { id, installment: installment_num, amount: amount_per_installment },
        );
    }

    // ==================== استعلامات محرك السيولة ====================

    pub fn get_liquidity_pool(env: Env) -> LiquidityPoolState {
        Self::get_liquidity_pool_state(&env)
    }

    pub fn get_total_available_liquidity(env: Env) -> i128 {
        let pool = Self::get_liquidity_pool_state(&env);
        pool.total_pooled
    }

    // ==================== دوال الاستعلام الأساسية ====================
    pub fn get_reputation(env: Env, user: Address) -> i128 { Self::get_reputation_internal(&env, &user) }
    pub fn get_commitment_pub(env: Env, id: u64) -> Commitment { Self::get_commitment(&env, id) }
    pub fn get_dispute(env: Env, dispute_id: u64) -> Dispute { env.storage().persistent().get(&DataKey::Dispute(dispute_id)).expect("Dispute not found") }
    pub fn get_contract_counter(env: Env) -> u64 { env.storage().persistent().get(&DataKey::ContractCounter).unwrap_or(0u64) }
    pub fn get_dispute_counter(env: Env) -> u64 { env.storage().persistent().get(&DataKey::DisputeCounter).unwrap_or(0u64) }
    pub fn get_contract_admin(env: Env) -> Address { env.storage().persistent().get(&DataKey::Admin).expect("Admin not set") }

    pub fn get_mithaq_forfeiture_split(_env: Env) -> (i128, i128, i128) {
        (FORFEITURE_PLATFORM_SHARE, FORFEITURE_SAFETY_SHARE, FORFEITURE_DAO_SHARE)
    }
}
