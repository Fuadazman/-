#![no_std]
/**
 * © جميع الحقوق محفوظة 2026 - المطور: فؤاد يحيى عزمان
 * البريد الإلكتروني: fuad.mithaq@gmail.com
 * Pi Chat: @Fuad207
 * مشروع: ميثاق (Mithaq) - العقد الذكي الرابع: الحوكمة اللامركزية (v2.0.0)
 * الملف: contracts/src/governance.rs
 * الوصف: إدارة الاقتراحات والتصويت وتوزيع الرسوم المصادرة 50/40/10 
 * (منصة / صندوق أمان / خزينة DAO) + انتخاب 5 أعضاء لخزينة DAO.
 * Developed & Architected by: Fuad Azman
 */
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, String, Symbol, Vec, Map,
    token::TokenClient,
};

// ===== أنواع البيانات =====

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GovernanceDataKey {
    Admin,
    ProposalCounter,
    Proposal(u64),
    Vote(u64, Address),
    MithaqContract,
    AzmanToken,
    PlatformTreasury,
    SafetyFund,
    DAOTreasury,
    VoteCount(u64, Symbol),
    DAOMember(Address),
    DAOMemberVote(Address, Address),
    DAOElectionStart,
    DAOElectionEnd,
    DAOElectionActive,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub title: String,
    pub description: String,
    pub status: Symbol,
    pub created_at: u64,
    pub discussion_end: u64,
    pub voting_start: u64,
    pub voting_end: u64,
    pub votes_yes: i128,
    pub votes_no: i128,
    pub total_voters: i128,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteRecord {
    pub proposal_id: u64,
    pub voter: Address,
    pub vote: Symbol,
    pub voted_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DAOMemberRecord {
    pub member: Address,
    pub arbitrator_level: String,
    pub votes_received: i128,
    pub elected_at: u64,
    pub term_expires: u64,
}

// ===== الأحداث (بدون contractevent) =====

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalSubmitted {
    pub id: u64,
    pub proposer: Address,
    pub title: String,
    pub fee_deducted: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteCast {
    pub proposal_id: u64,
    pub voter: Address,
    pub vote: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalApproved {
    pub id: u64,
    pub proposer: Address,
    pub refund_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalRejected {
    pub id: u64,
    pub proposer: Address,
    pub forfeited_amount: i128,
    pub platform_share: i128,
    pub safety_fund_share: i128,
    pub dao_treasury_share: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalStatusChanged {
    pub id: u64,
    pub old_status: Symbol,
    pub new_status: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DAOMemberElected {
    pub member: Address,
    pub votes_received: i128,
    pub term_expires: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DAOElectionStarted {
    pub start_time: u64,
    pub end_time: u64,
}

// ===== الثوابت =====

const PROPOSAL_FEE: i128 = 10_0000000;
const DISCUSSION_DURATION: u64 = 172_800;
const VOTING_DURATION: u64 = 604_800;
const APPROVAL_THRESHOLD: i128 = 60;

const FORFEITURE_PLATFORM_SHARE: i128 = 50;
const FORFEITURE_SAFETY_SHARE: i128 = 40;
const FORFEITURE_DAO_SHARE: i128 = 10;

const DAO_MEMBER_COUNT: u32 = 5;
const DAO_ELECTION_DURATION: u64 = 604_800;
const DAO_TERM_DURATION: u64 = 15_768_000;
const DAO_MIN_VOTES_TO_WIN: i128 = 10;
const DAO_MULTISIG_THRESHOLD: u32 = 3;

const S_DISCUSSION: Symbol = symbol_short!("DISCS");
const S_VOTING: Symbol = symbol_short!("VOTING");
const S_APPROVED: Symbol = symbol_short!("APPRV");
const S_REJECTED: Symbol = symbol_short!("REJCT");
const S_EXPIRED: Symbol = symbol_short!("EXPIR");

const S_YES: Symbol = symbol_short!("YES");
const S_NO: Symbol = symbol_short!("NO");

const LEDGERS_IN_30_DAYS: u32 = 518_400;

// ===== دوال مساعدة =====

fn extend_ttl(env: &Env, key: &GovernanceDataKey) {
    env.storage()
       .persistent()
       .extend_ttl(key, LEDGERS_IN_30_DAYS, LEDGERS_IN_30_DAYS);
}

fn get_proposal(env: &Env, id: u64) -> Proposal {
    let key = GovernanceDataKey::Proposal(id);
    let proposal: Proposal = env.storage()
        .persistent()
        .get(&key)
        .expect("Proposal not found");
    extend_ttl(env, &key);
    proposal
}

fn save_proposal(env: &Env, id: u64, proposal: &Proposal) {
    let key = GovernanceDataKey::Proposal(id);
    env.storage().persistent().set(&key, proposal);
    extend_ttl(env, &key);
}

fn get_admin(env: &Env) -> Address {
    let key = GovernanceDataKey::Admin;
    let admin: Address = env.storage()
        .persistent()
        .get(&key)
        .expect("Admin not set");
    extend_ttl(env, &key);
    admin
}

fn get_platform_treasury(env: &Env) -> Address {
    let key = GovernanceDataKey::PlatformTreasury;
    let treasury: Address = env.storage()
        .persistent()
        .get(&key)
        .expect("Platform Treasury not set");
    extend_ttl(env, &key);
    treasury
}

fn get_safety_fund(env: &Env) -> Address {
    let key = GovernanceDataKey::SafetyFund;
    let fund: Address = env.storage()
        .persistent()
        .get(&key)
        .expect("Safety Fund not set");
    extend_ttl(env, &key);
    fund
}

fn get_dao_treasury(env: &Env) -> Address {
    let key = GovernanceDataKey::DAOTreasury;
    let treasury: Address = env.storage()
        .persistent()
        .get(&key)
        .expect("DAO Treasury not set");
    extend_ttl(env, &key);
    treasury
}

fn get_azman_token(env: &Env) -> Address {
    let key = GovernanceDataKey::AzmanToken;
    let token: Address = env.storage()
        .persistent()
        .get(&key)
        .expect("Azman Token not set");
    extend_ttl(env, &key);
    token
}

fn transfer_azman(env: &Env, to: &Address, amount: i128) {
    if amount <= 0 {
        return;
    }
    let token_addr = get_azman_token(env);
    let token_client = TokenClient::new(env, &token_addr);
    token_client.transfer(&env.current_contract_address(), to, &amount);
}

// ===== العقد الرئيسي =====

#[contract]
pub struct MithaqGovernance;

#[contractimpl]
impl MithaqGovernance {

    // ==================== 1. التهيئة ====================

    pub fn initialize_governance(
        env: Env,
        admin: Address,
        mithaq_contract: Address,
        azman_token: Address,
        platform_treasury: Address,
        safety_fund: Address,
        dao_treasury: Address,
    ) {
        if env.storage().persistent().has(&GovernanceDataKey::Admin) {
            panic!("Governance contract already initialized");
        }
        admin.require_auth();

        env.storage().persistent().set(&GovernanceDataKey::Admin, &admin);
        env.storage().persistent().set(&GovernanceDataKey::MithaqContract, &mithaq_contract);
        env.storage().persistent().set(&GovernanceDataKey::AzmanToken, &azman_token);
        env.storage().persistent().set(&GovernanceDataKey::PlatformTreasury, &platform_treasury);
        env.storage().persistent().set(&GovernanceDataKey::SafetyFund, &safety_fund);
        env.storage().persistent().set(&GovernanceDataKey::DAOTreasury, &dao_treasury);
        env.storage().persistent().set(&GovernanceDataKey::ProposalCounter, &0u64);
        env.storage().persistent().set(&GovernanceDataKey::DAOElectionActive, &false);

        extend_ttl(&env, &GovernanceDataKey::Admin);
        extend_ttl(&env, &GovernanceDataKey::MithaqContract);
        extend_ttl(&env, &GovernanceDataKey::AzmanToken);
        extend_ttl(&env, &GovernanceDataKey::PlatformTreasury);
        extend_ttl(&env, &GovernanceDataKey::SafetyFund);
        extend_ttl(&env, &GovernanceDataKey::DAOTreasury);
    }

    // ==================== 2. تقديم اقتراح جديد ====================

    pub fn submit_proposal(
        env: Env,
        proposer: Address,
        title: String,
        description: String,
    ) -> u64 {
        proposer.require_auth();

        if title.is_empty() || description.is_empty() {
            panic!("Title and description are required");
        }

        let token_addr = get_azman_token(&env);
        let token_client = TokenClient::new(&env, &token_addr);

        let proposer_balance = token_client.balance(&proposer);
        if proposer_balance < PROPOSAL_FEE {
            panic!("Insufficient Azman balance. Required: 10 Azman");
        }

        token_client.transfer(&proposer, &env.current_contract_address(), &PROPOSAL_FEE);

        let counter_key = GovernanceDataKey::ProposalCounter;
        let counter: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0) + 1;
        env.storage().persistent().set(&counter_key, &counter);
        extend_ttl(&env, &counter_key);

        let now = env.ledger().timestamp();
        let discussion_end = now + DISCUSSION_DURATION;
        let voting_start = discussion_end;
        let voting_end = voting_start + VOTING_DURATION;

        let proposal = Proposal {
            id: counter,
            proposer: proposer.clone(),
            title: title.clone(),
            description,
            status: S_DISCUSSION,
            created_at: now,
            discussion_end,
            voting_start,
            voting_end,
            votes_yes: 0,
            votes_no: 0,
            total_voters: 0,
            executed: false,
        };

        save_proposal(&env, counter, &proposal);

        env.events().publish(
            (Symbol::new(&env, "proposal_submitted"),),
            ProposalSubmitted {
                id: counter,
                proposer,
                title,
                fee_deducted: PROPOSAL_FEE,
            },
        );

        counter
    }

    // ==================== 3. التصويت على اقتراح ====================

    pub fn vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        vote_type: Symbol,
    ) {
        voter.require_auth();

        if vote_type != S_YES && vote_type != S_NO {
            panic!("Vote must be YES or NO");
        }

        let mut proposal = get_proposal(&env, proposal_id);

        if proposal.status != S_VOTING {
            panic!("Proposal is not in voting phase");
        }

        let now = env.ledger().timestamp();
        if now > proposal.voting_end {
            proposal.status = S_EXPIRED;
            save_proposal(&env, proposal_id, &proposal);
            env.events().publish(
                (Symbol::new(&env, "proposal_status_changed"),),
                ProposalStatusChanged {
                    id: proposal_id,
                    old_status: S_VOTING,
                    new_status: S_EXPIRED,
                },
            );
            panic!("Voting period has ended");
        }

        let vote_key = GovernanceDataKey::Vote(proposal_id, voter.clone());
        if env.storage().persistent().has(&vote_key) {
            panic!("You have already voted on this proposal");
        }

        let vote_record = VoteRecord {
            proposal_id,
            voter: voter.clone(),
            vote: vote_type.clone(),
            voted_at: now,
        };
        env.storage().persistent().set(&vote_key, &vote_record);
        extend_ttl(&env, &vote_key);

        if vote_type == S_YES {
            proposal.votes_yes += 1;
        } else {
            proposal.votes_no += 1;
        }
        proposal.total_voters += 1;

        save_proposal(&env, proposal_id, &proposal);

        env.events().publish(
            (Symbol::new(&env, "vote_cast"),),
            VoteCast {
                proposal_id,
                voter,
                vote: vote_type,
            },
        );
    }

    // ==================== 4. تنفيذ نتيجة الاقتراح ====================

    pub fn execute_proposal(
        env: Env,
        proposal_id: u64,
    ) {
        let mut proposal = get_proposal(&env, proposal_id);

        if proposal.executed {
            panic!("Proposal already executed");
        }

        let now = env.ledger().timestamp();

        if proposal.status == S_DISCUSSION && now >= proposal.voting_start {
            proposal.status = S_VOTING;
            save_proposal(&env, proposal_id, &proposal);
            env.events().publish(
                (Symbol::new(&env, "proposal_status_changed"),),
                ProposalStatusChanged {
                    id: proposal_id,
                    old_status: S_DISCUSSION,
                    new_status: S_VOTING,
                },
            );
            panic!("Proposal moved to voting phase. Execute after voting ends.");
        }

        if proposal.status == S_VOTING && now < proposal.voting_end {
            panic!("Voting period is still active");
        }

        let total_votes = proposal.votes_yes + proposal.votes_no;
        let approval_percentage = if total_votes > 0 {
            (proposal.votes_yes * 100) / total_votes
        } else {
            0
        };

        if approval_percentage >= APPROVAL_THRESHOLD && total_votes > 0 {
            proposal.status = S_APPROVED;
            proposal.executed = true;
            save_proposal(&env, proposal_id, &proposal);

            transfer_azman(&env, &proposal.proposer, PROPOSAL_FEE);

            env.events().publish(
                (Symbol::new(&env, "proposal_approved"),),
                ProposalApproved {
                    id: proposal_id,
                    proposer: proposal.proposer.clone(),
                    refund_amount: PROPOSAL_FEE,
                },
            );

            env.events().publish(
                (Symbol::new(&env, "proposal_status_changed"),),
                ProposalStatusChanged {
                    id: proposal_id,
                    old_status: S_VOTING,
                    new_status: S_APPROVED,
                },
            );
        } else {
            proposal.status = S_REJECTED;
            proposal.executed = true;
            save_proposal(&env, proposal_id, &proposal);

            let platform_share = (PROPOSAL_FEE * FORFEITURE_PLATFORM_SHARE) / 100;
            let safety_share = (PROPOSAL_FEE * FORFEITURE_SAFETY_SHARE) / 100;
            let dao_share = (PROPOSAL_FEE * FORFEITURE_DAO_SHARE) / 100;

            let platform_treasury = get_platform_treasury(&env);
            let safety_fund = get_safety_fund(&env);
            let dao_treasury = get_dao_treasury(&env);

            transfer_azman(&env, &platform_treasury, platform_share);
            transfer_azman(&env, &safety_fund, safety_share);
            transfer_azman(&env, &dao_treasury, dao_share);

            env.events().publish(
                (Symbol::new(&env, "proposal_rejected"),),
                ProposalRejected {
                    id: proposal_id,
                    proposer: proposal.proposer.clone(),
                    forfeited_amount: PROPOSAL_FEE,
                    platform_share,
                    safety_fund_share: safety_share,
                    dao_treasury_share: dao_share,
                },
            );

            env.events().publish(
                (Symbol::new(&env, "proposal_status_changed"),),
                ProposalStatusChanged {
                    id: proposal_id,
                    old_status: S_VOTING,
                    new_status: S_REJECTED,
                },
            );
        }
    }

    // ==================== 5. انتخاب أعضاء خزينة DAO ====================

    pub fn start_dao_election(env: Env, admin: Address) {
        let stored_admin = get_admin(&env);
        if admin != stored_admin {
            panic!("Unauthorized: Only Admin");
        }
        admin.require_auth();

        let election_active: bool = env.storage().persistent()
            .get(&GovernanceDataKey::DAOElectionActive)
            .unwrap_or(false);
        if election_active {
            panic!("Election already in progress");
        }

        let now = env.ledger().timestamp();
        let end_time = now + DAO_ELECTION_DURATION;

        env.storage().persistent().set(&GovernanceDataKey::DAOElectionActive, &true);
        env.storage().persistent().set(&GovernanceDataKey::DAOElectionStart, &now);
        env.storage().persistent().set(&GovernanceDataKey::DAOElectionEnd, &end_time);

        env.events().publish(
            (Symbol::new(&env, "dao_election_started"),),
            DAOElectionStarted { start_time: now, end_time },
        );
    }

    pub fn vote_for_dao_member(env: Env, voter: Address, candidate: Address) {
        voter.require_auth();

        let election_active: bool = env.storage().persistent()
            .get(&GovernanceDataKey::DAOElectionActive)
            .unwrap_or(false);
        if !election_active {
            panic!("No active election");
        }

        let now = env.ledger().timestamp();
        let end_time: u64 = env.storage().persistent()
            .get(&GovernanceDataKey::DAOElectionEnd)
            .expect("Election end time not set");
        if now > end_time {
            panic!("Election has ended");
        }

        let vote_key = GovernanceDataKey::DAOMemberVote(voter.clone(), candidate.clone());
        if env.storage().persistent().has(&vote_key) {
            panic!("You have already voted for this candidate");
        }

        env.storage().persistent().set(&vote_key, &true);
        extend_ttl(&env, &vote_key);

        let member_key = GovernanceDataKey::DAOMember(candidate.clone());
        let current_votes: i128 = env.storage().persistent().get(&member_key).unwrap_or(0);
        env.storage().persistent().set(&member_key, &(current_votes + 1));
        extend_ttl(&env, &member_key);
    }

    pub fn finalize_dao_election(env: Env, admin: Address) {
        let stored_admin = get_admin(&env);
        if admin != stored_admin {
            panic!("Unauthorized: Only Admin");
        }
        admin.require_auth();

        let election_active: bool = env.storage().persistent()
            .get(&GovernanceDataKey::DAOElectionActive)
            .unwrap_or(false);
        if !election_active {
            panic!("No active election to finalize");
        }

        let now = env.ledger().timestamp();
        let end_time: u64 = env.storage().persistent()
            .get(&GovernanceDataKey::DAOElectionEnd)
            .expect("Election end time not set");
        if now < end_time {
            panic!("Election period has not ended yet");
        }

        env.storage().persistent().set(&GovernanceDataKey::DAOElectionActive, &false);

        let term_expires = now + DAO_TERM_DURATION;
    }

    pub fn dao_treasury_disburse(
        env: Env,
        proposal_id: u64,
        amount: i128,
        recipient: Address,
        signatures: Vec<Address>,
    ) {
        if signatures.len() < DAO_MULTISIG_THRESHOLD {
            panic!("Insufficient signatures. Required: 3 of 5");
        }

        for signer in signatures.iter() {
            signer.require_auth();
            let member_key = GovernanceDataKey::DAOMember(signer);
            if !env.storage().persistent().has(&member_key) {
                panic!("Unauthorized signer: Not a DAO member");
            }
        }

        let dao_treasury = get_dao_treasury(&env);
        let token_client = TokenClient::new(&env, &get_azman_token(&env));
        let treasury_balance = token_client.balance(&dao_treasury);
        if treasury_balance < amount {
            panic!("Insufficient DAO treasury balance");
        }

        token_client.transfer(&env.current_contract_address(), &recipient, &amount);
    }

    pub fn get_dao_member_count(env: Env) -> u32 {
        DAO_MEMBER_COUNT
    }

    pub fn get_dao_multisig_threshold(env: Env) -> u32 {
        DAO_MULTISIG_THRESHOLD
    }

    // ==================== 6. وظائف إدارية ====================

    pub fn update_gov_mithaq_contract(env: Env, admin: Address, new_contract: Address) {
        let stored_admin = get_admin(&env);
        if admin != stored_admin {
            panic!("Unauthorized: Only Admin");
        }
        admin.require_auth();

        env.storage().persistent().set(&GovernanceDataKey::MithaqContract, &new_contract);
        extend_ttl(&env, &GovernanceDataKey::MithaqContract);
    }

    pub fn update_gov_azman_token(env: Env, admin: Address, new_token: Address) {
        let stored_admin = get_admin(&env);
        if admin != stored_admin {
            panic!("Unauthorized: Only Admin");
        }
        admin.require_auth();

        env.storage().persistent().set(&GovernanceDataKey::AzmanToken, &new_token);
        extend_ttl(&env, &GovernanceDataKey::AzmanToken);
    }

    pub fn update_gov_platform_treasury(env: Env, admin: Address, new_treasury: Address) {
        let stored_admin = get_admin(&env);
        if admin != stored_admin {
            panic!("Unauthorized: Only Admin");
        }
        admin.require_auth();

        env.storage().persistent().set(&GovernanceDataKey::PlatformTreasury, &new_treasury);
        extend_ttl(&env, &GovernanceDataKey::PlatformTreasury);
    }

    pub fn update_gov_safety_fund(env: Env, admin: Address, new_fund: Address) {
        let stored_admin = get_admin(&env);
        if admin != stored_admin {
            panic!("Unauthorized: Only Admin");
        }
        admin.require_auth();

        env.storage().persistent().set(&GovernanceDataKey::SafetyFund, &new_fund);
        extend_ttl(&env, &GovernanceDataKey::SafetyFund);
    }

    pub fn update_gov_dao_treasury(env: Env, admin: Address, new_treasury: Address) {
        let stored_admin = get_admin(&env);
        if admin != stored_admin {
            panic!("Unauthorized: Only Admin");
        }
        admin.require_auth();

        env.storage().persistent().set(&GovernanceDataKey::DAOTreasury, &new_treasury);
        extend_ttl(&env, &GovernanceDataKey::DAOTreasury);
    }

    pub fn transfer_governance_admin(env: Env, current_admin: Address, new_admin: Address) {
        let stored_admin = get_admin(&env);
        if current_admin != stored_admin {
            panic!("Unauthorized: Only current Admin");
        }
        current_admin.require_auth();
        new_admin.require_auth();

        env.storage().persistent().set(&GovernanceDataKey::Admin, &new_admin);
        extend_ttl(&env, &GovernanceDataKey::Admin);
    }

    // ==================== 7. دوال الاستعلام ====================

    pub fn get_proposal_details(env: Env, proposal_id: u64) -> Proposal {
        get_proposal(&env, proposal_id)
    }

    pub fn has_voted(env: Env, proposal_id: u64, voter: Address) -> bool {
        let vote_key = GovernanceDataKey::Vote(proposal_id, voter);
        env.storage().persistent().has(&vote_key)
    }

    pub fn get_proposal_counter(env: Env) -> u64 {
        let key = GovernanceDataKey::ProposalCounter;
        let counter: u64 = env.storage().persistent().get(&key).unwrap_or(0);
        extend_ttl(&env, &key);
        counter
    }

    pub fn get_admin_address(env: Env) -> Address {
        get_admin(&env)
    }

    pub fn get_contract_azman_balance(env: Env) -> i128 {
        let token_addr = get_azman_token(&env);
        let token_client = TokenClient::new(&env, &token_addr);
        token_client.balance(&env.current_contract_address())
    }

    pub fn get_forfeiture_split(env: Env) -> (i128, i128, i128) {
        (FORFEITURE_PLATFORM_SHARE, FORFEITURE_SAFETY_SHARE, FORFEITURE_DAO_SHARE)
    }

    pub fn is_election_active(env: Env) -> bool {
        env.storage().persistent()
            .get(&GovernanceDataKey::DAOElectionActive)
            .unwrap_or(false)
    }

    pub fn get_election_end_time(env: Env) -> u64 {
        env.storage().persistent()
            .get(&GovernanceDataKey::DAOElectionEnd)
            .unwrap_or(0)
    }
}
