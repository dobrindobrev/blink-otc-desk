//! LiteSVM tests for the OTC desk. Run `anchor build` (or `cargo build-sbf`) first.

// LiteSVM's TransactionResult carries full metadata in its error variant.
#![allow(clippy::result_large_err)]

use anchor_lang::{prelude::Pubkey, AccountDeserialize, InstructionData, ToAccountMetas};
use litesvm::{types::TransactionResult, LiteSVM};
use otc_desk::{constants::OFFER_SEED, state::Offer};
use solana_keypair::Keypair;
use solana_message::{Instruction, Message};
use solana_program_pack::Pack;
use solana_signer::Signer;
use solana_transaction::Transaction;
use spl_associated_token_account_interface::{
    address::get_associated_token_address_with_program_id,
    instruction::create_associated_token_account_idempotent,
};
use spl_token_2022_interface::extension::{transfer_fee, ExtensionType};

const DECIMALS: u8 = 6;
const TOKEN: u64 = 10u64.pow(DECIMALS as u32);
const NOW: i64 = 1_800_000_000;
const HOUR: i64 = 3_600;
const NO_EXPIRY: i64 = 0;

const SPL_TOKEN: Pubkey = spl_token_interface::ID;
const TOKEN_2022: Pubkey = spl_token_2022_interface::ID;

fn program_bytes() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy/otc_desk.so");
    std::fs::read(path).expect("target/deploy/otc_desk.so not found - run `anchor build` first")
}

/// A mint plus the token program that owns it.
#[derive(Clone, Copy)]
struct Token {
    mint: Pubkey,
    program: Pubkey,
}

impl Token {
    fn ata(&self, owner: &Pubkey) -> Pubkey {
        get_associated_token_address_with_program_id(owner, &self.mint, &self.program)
    }
}

struct Env {
    svm: LiteSVM,
    issuer: Keypair,
    alice: Keypair,
    bob: Keypair,
    carol: Keypair,
    token_a: Token,
    token_b: Token,
}

fn send(svm: &mut LiteSVM, instructions: &[Instruction], signers: &[&Keypair]) -> TransactionResult {
    svm.expire_blockhash();
    let message = Message::new(instructions, Some(&signers[0].pubkey()));
    let transaction = Transaction::new(signers, message, svm.latest_blockhash());
    svm.send_transaction(transaction)
}

fn assert_error(result: TransactionResult, needle: &str) {
    let failure = result.expect_err("transaction should have failed");
    let logs = failure.meta.logs.join("\n");
    assert!(
        logs.contains(needle),
        "expected logs to contain {needle:?}, got:\n{logs}"
    );
}

fn set_time(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<anchor_lang::prelude::Clock>();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar(&clock);
}

fn balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    svm.get_account(token_account)
        .filter(|account| account.lamports > 0)
        // amount lives at bytes 64..72 of the base token account layout
        .map_or(0, |account| {
            u64::from_le_bytes(account.data[64..72].try_into().unwrap())
        })
}

fn is_closed(svm: &LiteSVM, address: &Pubkey) -> bool {
    svm.get_account(address)
        .is_none_or(|account| account.lamports == 0)
}

fn offer_pda(maker: &Pubkey, id: u64) -> Pubkey {
    Pubkey::find_program_address(&[OFFER_SEED, maker.as_ref(), &id.to_le_bytes()], &otc_desk::ID).0
}

fn read_offer(svm: &LiteSVM, address: &Pubkey) -> Offer {
    let account = svm.get_account(address).expect("offer exists");
    Offer::try_deserialize(&mut account.data.as_slice()).unwrap()
}

/// Creates a mint owned by `program`. With `transfer_fee_bps`, the mint is a
/// Token-2022 mint with a transfer fee.
fn create_mint(svm: &mut LiteSVM, payer: &Keypair, program: Pubkey, transfer_fee_bps: Option<u16>) -> Token {
    let mint = Keypair::new();
    let extensions: &[ExtensionType] = if transfer_fee_bps.is_some() {
        &[ExtensionType::TransferFeeConfig]
    } else {
        &[]
    };
    let space = if program == TOKEN_2022 {
        ExtensionType::try_calculate_account_len::<spl_token_2022_interface::state::Mint>(extensions).unwrap()
    } else {
        spl_token_interface::state::Mint::LEN
    };
    let mut instructions = vec![solana_system_interface::instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        svm.minimum_balance_for_rent_exemption(space),
        space as u64,
        &program,
    )];
    if let Some(bps) = transfer_fee_bps {
        instructions.push(
            transfer_fee::instruction::initialize_transfer_fee_config(
                &TOKEN_2022,
                &mint.pubkey(),
                None,
                None,
                bps,
                u64::MAX,
            )
            .unwrap(),
        );
    }
    instructions.push(if program == TOKEN_2022 {
        spl_token_2022_interface::instruction::initialize_mint2(
            &program,
            &mint.pubkey(),
            &payer.pubkey(),
            None,
            DECIMALS,
        )
        .unwrap()
    } else {
        spl_token_interface::instruction::initialize_mint2(
            &program,
            &mint.pubkey(),
            &payer.pubkey(),
            None,
            DECIMALS,
        )
        .unwrap()
    });
    send(svm, &instructions, &[payer, &mint]).expect("create mint");
    Token {
        mint: mint.pubkey(),
        program,
    }
}

fn fund(svm: &mut LiteSVM, issuer: &Keypair, token: Token, owner: &Pubkey, amount: u64) {
    let ata = token.ata(owner);
    let mint_to = if token.program == TOKEN_2022 {
        spl_token_2022_interface::instruction::mint_to(
            &token.program,
            &token.mint,
            &ata,
            &issuer.pubkey(),
            &[],
            amount,
        )
    } else {
        spl_token_interface::instruction::mint_to(
            &token.program,
            &token.mint,
            &ata,
            &issuer.pubkey(),
            &[],
            amount,
        )
    }
    .unwrap();
    let instructions = [
        create_associated_token_account_idempotent(&issuer.pubkey(), owner, &token.mint, &token.program),
        mint_to,
    ];
    send(svm, &instructions, &[issuer]).expect("fund");
}

fn setup_with(token_a_program: Pubkey, token_a_fee_bps: Option<u16>) -> Env {
    let mut svm = LiteSVM::new();
    svm.add_program(otc_desk::ID, &program_bytes()).unwrap();
    set_time(&mut svm, NOW);

    let [issuer, alice, bob, carol] = [Keypair::new(), Keypair::new(), Keypair::new(), Keypair::new()];
    for keypair in [&issuer, &alice, &bob, &carol] {
        svm.airdrop(&keypair.pubkey(), 10_000_000_000).unwrap();
    }

    let token_a = create_mint(&mut svm, &issuer, token_a_program, token_a_fee_bps);
    let token_b = create_mint(&mut svm, &issuer, SPL_TOKEN, None);
    fund(&mut svm, &issuer, token_a, &alice.pubkey(), 1_000 * TOKEN);
    fund(&mut svm, &issuer, token_a, &bob.pubkey(), 1_000 * TOKEN);
    fund(&mut svm, &issuer, token_b, &bob.pubkey(), 1_000 * TOKEN);
    fund(&mut svm, &issuer, token_b, &carol.pubkey(), 1_000 * TOKEN);

    Env {
        svm,
        issuer,
        alice,
        bob,
        carol,
        token_a,
        token_b,
    }
}

/// Classic SPL Token on both sides.
fn setup() -> Env {
    setup_with(SPL_TOKEN, None)
}

struct OfferTerms {
    id: u64,
    offered: u64,
    wanted: u64,
    allow_partial_fills: bool,
    expires_at: i64,
}

impl Default for OfferTerms {
    fn default() -> Self {
        Self {
            id: 1,
            offered: 100 * TOKEN,
            wanted: 250 * TOKEN,
            allow_partial_fills: false,
            expires_at: NO_EXPIRY,
        }
    }
}

fn make_offer_ix(maker: &Pubkey, token_a: Token, token_b: Token, terms: &OfferTerms) -> Instruction {
    let offer = offer_pda(maker, terms.id);
    Instruction {
        program_id: otc_desk::ID,
        accounts: otc_desk::accounts::MakeOffer {
            associated_token_program: spl_associated_token_account_interface::program::ID,
            token_program_a: token_a.program,
            token_program_b: token_b.program,
            system_program: solana_system_interface::program::ID,
            maker: *maker,
            token_mint_a: token_a.mint,
            token_mint_b: token_b.mint,
            maker_token_account_a: token_a.ata(maker),
            offer,
            vault: token_a.ata(&offer),
        }
        .to_account_metas(None),
        data: otc_desk::instruction::MakeOffer {
            id: terms.id,
            token_a_offered_amount: terms.offered,
            token_b_wanted_amount: terms.wanted,
            allow_partial_fills: terms.allow_partial_fills,
            expires_at: terms.expires_at,
        }
        .data(),
    }
}

fn take_offer_ix(
    taker: &Pubkey,
    maker: &Pubkey,
    id: u64,
    token_a: Token,
    token_b: Token,
    token_a_amount: u64,
    max_token_b_amount: u64,
) -> Instruction {
    let offer = offer_pda(maker, id);
    Instruction {
        program_id: otc_desk::ID,
        accounts: otc_desk::accounts::TakeOffer {
            associated_token_program: spl_associated_token_account_interface::program::ID,
            token_program_a: token_a.program,
            token_program_b: token_b.program,
            system_program: solana_system_interface::program::ID,
            taker: *taker,
            maker: *maker,
            token_mint_a: token_a.mint,
            token_mint_b: token_b.mint,
            taker_token_account_a: token_a.ata(taker),
            taker_token_account_b: token_b.ata(taker),
            maker_token_account_b: token_b.ata(maker),
            offer,
            vault: token_a.ata(&offer),
        }
        .to_account_metas(None),
        data: otc_desk::instruction::TakeOffer {
            token_a_amount,
            max_token_b_amount,
        }
        .data(),
    }
}

fn refund_offer_ix(caller: &Pubkey, maker: &Pubkey, id: u64, token_a: Token) -> Instruction {
    let offer = offer_pda(maker, id);
    Instruction {
        program_id: otc_desk::ID,
        accounts: otc_desk::accounts::RefundOffer {
            associated_token_program: spl_associated_token_account_interface::program::ID,
            token_program_a: token_a.program,
            system_program: solana_system_interface::program::ID,
            caller: *caller,
            maker: *maker,
            token_mint_a: token_a.mint,
            maker_token_account_a: token_a.ata(maker),
            offer,
            vault: token_a.ata(&offer),
        }
        .to_account_metas(None),
        data: otc_desk::instruction::RefundOffer {}.data(),
    }
}

impl Env {
    fn alice_makes(&mut self, terms: &OfferTerms) -> TransactionResult {
        let alice = self.alice.insecure_clone();
        let instruction = make_offer_ix(&alice.pubkey(), self.token_a, self.token_b, terms);
        send(&mut self.svm, &[instruction], &[&alice])
    }

    fn takes(
        &mut self,
        taker: &Keypair,
        id: u64,
        token_a_amount: u64,
        max_token_b_amount: u64,
    ) -> TransactionResult {
        let instruction = take_offer_ix(
            &taker.pubkey(),
            &self.alice.pubkey(),
            id,
            self.token_a,
            self.token_b,
            token_a_amount,
            max_token_b_amount,
        );
        send(&mut self.svm, &[instruction], &[taker])
    }

    fn refunds(&mut self, caller: &Keypair, id: u64) -> TransactionResult {
        let instruction = refund_offer_ix(&caller.pubkey(), &self.alice.pubkey(), id, self.token_a);
        send(&mut self.svm, &[instruction], &[caller])
    }

    fn bob(&self) -> Keypair {
        self.bob.insecure_clone()
    }

    fn carol(&self) -> Keypair {
        self.carol.insecure_clone()
    }
}

#[test]
fn full_swap_moves_both_tokens_and_closes_everything() {
    let mut env = setup();
    let terms = OfferTerms::default();
    env.alice_makes(&terms).expect("make offer");

    let offer = offer_pda(&env.alice.pubkey(), terms.id);
    let vault = env.token_a.ata(&offer);
    assert_eq!(balance(&env.svm, &vault), terms.offered);

    let bob = env.bob();
    env.takes(&bob, terms.id, terms.offered, terms.wanted)
        .expect("take offer");

    assert_eq!(
        balance(&env.svm, &env.token_a.ata(&env.alice.pubkey())),
        900 * TOKEN
    );
    assert_eq!(
        balance(&env.svm, &env.token_b.ata(&env.alice.pubkey())),
        terms.wanted
    );
    assert_eq!(balance(&env.svm, &env.token_a.ata(&bob.pubkey())), 1_100 * TOKEN);
    assert_eq!(balance(&env.svm, &env.token_b.ata(&bob.pubkey())), 750 * TOKEN);
    assert!(is_closed(&env.svm, &offer), "offer closed");
    assert!(is_closed(&env.svm, &vault), "vault closed");
}

#[test]
fn offer_ids_are_scoped_per_maker() {
    // Upstream derived offers from ["offer", id] only, so anyone could squat
    // another maker's ID. Now two makers can both use ID 1.
    let mut env = setup();
    env.alice_makes(&OfferTerms::default()).expect("alice offer 1");
    let bob = env.bob();
    let instruction = make_offer_ix(&bob.pubkey(), env.token_a, env.token_b, &OfferTerms::default());
    send(&mut env.svm, &[instruction], &[&bob]).expect("bob offer 1");
}

#[test]
fn same_maker_cannot_reuse_an_open_id() {
    let mut env = setup();
    env.alice_makes(&OfferTerms::default()).unwrap();
    assert_error(env.alice_makes(&OfferTerms::default()), "already in use");
}

#[test]
fn partial_fills_are_pro_rata_and_the_last_fill_closes_the_offer() {
    let mut env = setup();
    // 3 token A for 10 token B: thirds don't divide evenly
    let terms = OfferTerms {
        offered: 3,
        wanted: 10,
        allow_partial_fills: true,
        ..Default::default()
    };
    env.alice_makes(&terms).unwrap();
    let offer = offer_pda(&env.alice.pubkey(), terms.id);
    let (bob, carol) = (env.bob(), env.carol());
    let alice_b = env.token_b.ata(&env.alice.pubkey());

    env.takes(&bob, terms.id, 1, 4).expect("first third, rounded up");
    assert_eq!(balance(&env.svm, &alice_b), 4);
    let state = read_offer(&env.svm, &offer);
    assert_eq!((state.token_a_remaining, state.token_b_received), (2, 4));

    env.takes(&carol, terms.id, 1, 4).expect("second third");
    env.takes(&bob, terms.id, 1, 2)
        .expect("last third settles the remainder");

    assert_eq!(
        balance(&env.svm, &alice_b),
        10,
        "maker gets exactly the asking price"
    );
    assert!(is_closed(&env.svm, &offer));
    assert!(is_closed(&env.svm, &env.token_a.ata(&offer)));
}

#[test]
fn partial_fills_must_be_enabled() {
    let mut env = setup();
    env.alice_makes(&OfferTerms::default()).unwrap();
    let bob = env.bob();
    assert_error(env.takes(&bob, 1, 10 * TOKEN, u64::MAX), "PartialFillNotAllowed");
}

#[test]
fn cannot_take_more_than_remains() {
    let mut env = setup();
    env.alice_makes(&OfferTerms {
        allow_partial_fills: true,
        ..Default::default()
    })
    .unwrap();
    let bob = env.bob();
    assert_error(
        env.takes(&bob, 1, 100 * TOKEN + 1, u64::MAX),
        "AmountExceedsRemaining",
    );
}

#[test]
fn taker_price_guard() {
    let mut env = setup();
    env.alice_makes(&OfferTerms::default()).unwrap();
    let bob = env.bob();
    assert_error(
        env.takes(&bob, 1, 100 * TOKEN, 250 * TOKEN - 1),
        "SlippageExceeded",
    );
}

#[test]
fn expired_offers_cannot_be_taken() {
    let mut env = setup();
    env.alice_makes(&OfferTerms {
        expires_at: NOW + HOUR,
        ..Default::default()
    })
    .unwrap();
    set_time(&mut env.svm, NOW + HOUR);
    let bob = env.bob();
    assert_error(env.takes(&bob, 1, 100 * TOKEN, u64::MAX), "OfferExpired");
}

#[test]
fn expiry_must_be_in_the_future() {
    let mut env = setup();
    assert_error(
        env.alice_makes(&OfferTerms {
            expires_at: NOW,
            ..Default::default()
        }),
        "InvalidExpiry",
    );
}

#[test]
fn anyone_can_refund_an_expired_offer_to_the_maker() {
    let mut env = setup();
    env.alice_makes(&OfferTerms {
        expires_at: NOW + HOUR,
        ..Default::default()
    })
    .unwrap();
    let carol = env.carol();

    assert_error(env.refunds(&carol, 1), "RefundNotAllowed");

    set_time(&mut env.svm, NOW + HOUR);
    env.refunds(&carol, 1).expect("crank refund after expiry");
    assert_eq!(
        balance(&env.svm, &env.token_a.ata(&env.alice.pubkey())),
        1_000 * TOKEN
    );
    assert!(is_closed(&env.svm, &offer_pda(&env.alice.pubkey(), 1)));
}

#[test]
fn maker_can_refund_the_unfilled_part_at_any_time() {
    let mut env = setup();
    env.alice_makes(&OfferTerms {
        allow_partial_fills: true,
        ..Default::default()
    })
    .unwrap();
    let bob = env.bob();
    env.takes(&bob, 1, 40 * TOKEN, u64::MAX).unwrap();

    let alice = env.alice.insecure_clone();
    env.refunds(&alice, 1).expect("maker refund");
    assert_eq!(balance(&env.svm, &env.token_a.ata(&alice.pubkey())), 960 * TOKEN);
    assert_eq!(balance(&env.svm, &env.token_b.ata(&alice.pubkey())), 100 * TOKEN);
}

#[test]
fn rejects_invalid_offers() {
    let mut env = setup();
    assert_error(
        env.alice_makes(&OfferTerms {
            offered: 0,
            ..Default::default()
        }),
        "InvalidAmount",
    );
    assert_error(
        env.alice_makes(&OfferTerms {
            wanted: 0,
            ..Default::default()
        }),
        "InvalidAmount",
    );
    assert_error(
        env.alice_makes(&OfferTerms {
            offered: 1_001 * TOKEN,
            ..Default::default()
        }),
        "InsufficientMakerBalance",
    );

    let alice = env.alice.insecure_clone();
    let same_mint = make_offer_ix(&alice.pubkey(), env.token_a, env.token_a, &OfferTerms::default());
    assert_error(send(&mut env.svm, &[same_mint], &[&alice]), "InvalidTokenMint");
}

#[test]
fn taker_cannot_substitute_a_different_token() {
    let mut env = setup();
    env.alice_makes(&OfferTerms::default()).unwrap();
    let issuer = env.issuer.insecure_clone();
    let fake_b = create_mint(&mut env.svm, &issuer, SPL_TOKEN, None);
    let bob = env.bob();
    fund(&mut env.svm, &issuer, fake_b, &bob.pubkey(), 1_000 * TOKEN);

    let instruction = take_offer_ix(
        &bob.pubkey(),
        &env.alice.pubkey(),
        1,
        env.token_a,
        fake_b,
        100 * TOKEN,
        u64::MAX,
    );
    assert_error(send(&mut env.svm, &[instruction], &[&bob]), "ConstraintHasOne");
}

#[test]
fn swaps_token_2022_with_a_transfer_fee_against_classic_spl_token() {
    // 1% transfer fee on token A (Token-2022), token B is classic SPL Token
    let mut env = setup_with(TOKEN_2022, Some(100));
    let terms = OfferTerms {
        allow_partial_fills: true,
        ..Default::default()
    };
    env.alice_makes(&terms)
        .expect("make offer with a fee-bearing token");

    // 1% of 100 was withheld on the way into the vault, so the offer is for 99
    let offer = read_offer(&env.svm, &offer_pda(&env.alice.pubkey(), 1));
    assert_eq!(offer.token_a_offered_amount, 99 * TOKEN);
    assert_eq!(offer.token_a_remaining, 99 * TOKEN);

    let bob = env.bob();
    env.takes(&bob, 1, 99 * TOKEN, 250 * TOKEN)
        .expect("take everything");
    assert_eq!(
        balance(&env.svm, &env.token_b.ata(&env.alice.pubkey())),
        250 * TOKEN
    );
    // Bob receives 99 minus another 1% fee on the way out of the vault
    assert_eq!(
        balance(&env.svm, &env.token_a.ata(&bob.pubkey())),
        1_000 * TOKEN + 99 * TOKEN - 990_000
    );
}

#[test]
fn fee_bearing_offers_can_be_refunded_and_closed() {
    // The vault holds withheld fees, which must be harvested before it can close
    let mut env = setup_with(TOKEN_2022, Some(100));
    env.alice_makes(&OfferTerms {
        allow_partial_fills: true,
        ..Default::default()
    })
    .unwrap();
    let bob = env.bob();
    env.takes(&bob, 1, 9 * TOKEN, u64::MAX).expect("partial take");

    let alice = env.alice.insecure_clone();
    env.refunds(&alice, 1)
        .expect("refund harvests fees and closes the vault");
    let offer = offer_pda(&alice.pubkey(), 1);
    assert!(is_closed(&env.svm, &offer));
    assert!(is_closed(&env.svm, &env.token_a.ata(&offer)));
}

#[test]
fn taker_needs_enough_token_b() {
    let mut env = setup();
    env.alice_makes(&OfferTerms {
        wanted: 1_001 * TOKEN,
        ..Default::default()
    })
    .unwrap();
    let bob = env.bob();
    assert_error(
        env.takes(&bob, 1, 100 * TOKEN, u64::MAX),
        "InsufficientTakerBalance",
    );
}
