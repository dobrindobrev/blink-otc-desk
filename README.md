# Blink OTC Desk

**Peer-to-peer OTC token offers on Solana that you can share as [Blinks](https://solana.com/docs/advanced/actions).** Post an offer like "500 TOKEN for 1,250 USDC" and share its link on X, Discord or Telegram. Anyone can fill it, in full or in part, straight from their wallet. Settlement happens atomically on-chain: there's no custody, no order book and no fees.

> Forked from [solanakite/anchor-escrow-2026](https://github.com/solanakite/anchor-escrow-2026) by Mike MacCana (MIT), a minimal teaching escrow. This fork turns it into an OTC desk. See [what changed](#what-i-added-on-top-of-upstream).

```
 maker                         OTC Desk program                        taker (via Blink)
   │  make_offer(id, 500 A → 1250 B,      │                                   │
   │    partial fills, expires in 48h)    │                                   │
   ├──── 500 A ─────────────────────────► vault (PDA-owned ATA)               │
   │                                      │   GET  /api/actions/offer/…  ◄────┤ wallet unfurls the link
   │                                      │   POST {account} → unsigned tx ──►│ taker signs
   │                                      │◄──────── take_offer(200 A, max 500 B)
   │◄──────────────── 500 B ──────────────┤────────── 200 A ────────────────►│
   │                    …more fills, or refund after expiry…                  │
```

## Features

| | |
|---|---|
| **Blink-native** | Each offer has a Solana Actions endpoint (`GET` metadata, `POST` → transaction) and an `actions.json`, so links unfurl into blinks |
| **Partial fills** | Optional, priced pro rata and rounded up for the maker. The final fill settles the remainder, so the maker always receives at least their price (property-tested) |
| **Price guard** | Takers sign with `max_token_b_amount`, so the transaction fails rather than overpays if an offer changes after the quote |
| **Expiry and crank refunds** | Offers can expire. After expiry, anyone can return the unfilled tokens to the maker |
| **SPL Token ↔ Token-2022** | Each side picks its own token program |
| **Transfer-fee aware** | Offers record what actually reached the vault, and withheld fees are harvested before the vault closes |
| **Events** | `OfferMade`, `OfferTaken` and `OfferClosed` for indexers |

## What I added on top of upstream

Upstream is an excellent minimal escrow for learning. I kept its structure and comments and changed the following:

1. **Migrated to Anchor 1.2.1** from 0.32.1. `CpiContext::new` now takes a program ID, the lockfile is regenerated, and `[registry]` is gone.
2. **Fixed an offer-ID squatting issue.** Upstream offers live at `["offer", id]`, so anyone can claim an ID first and block or front-run another maker. Offers now live at `["offer", maker, id]`, and `offer_ids_are_scoped_per_maker` covers it.
3. **Added partial fills, expiry, crank refunds, mixed token programs and a taker price guard.**
4. **Added Token-2022 transfer-fee support.** The tests found that a vault holding withheld fees can't be closed, which would leave fee-bearing offers stuck forever. Fees are now harvested to the mint first.
5. **Made errors meaningful.** Upstream wrapped token CPIs in `.map_err(|_| InsufficientMakerBalance)`, but a failed CPI aborts the whole transaction, so that mapping never runs. Balances are now checked up front.
6. **Built the Blink app** (Next.js 16 and `@solana/kit` 8, with a Codama-generated client) and a maker CLI.
7. **Rewrote the tests.** There are 16 LiteSVM tests, property tests for pricing, and end-to-end Blink tests where a "wallet" signs whatever the Blink returns against a real `solana-test-validator`.

The full list is in [CHANGELOG.md](CHANGELOG.md).

## Project layout

```
programs/otc_desk/
  src/handlers/        make_offer, take_offer, refund_offer, shared token helpers
  src/pricing.rs       partial-fill pricing (+ property tests)
  src/state/offer.rs   Offer account
  tests/otc_desk.rs    LiteSVM tests
app/                   Next.js Blink server + UI
  src/app/api/actions/ Solana Actions endpoints
  src/generated/       Codama client (generated from the IDL)
  scripts/make-offer.ts
  tests/               unit + end-to-end (local validator)
scripts/generate-client.mjs
```

## Getting started

Prerequisites: Rust, [Solana CLI](https://docs.anza.xyz/cli/install) 4.x, [Anchor](https://www.anchor-lang.com/docs/installation) 1.2.1 and Node 22.

```bash
npm install
anchor build                 # program + IDL
npm run test:program         # unit, property and LiteSVM tests
npm run test:app             # app unit tests
npm run test:e2e -w app      # Blink end-to-end against solana-test-validator
```

### Run it locally

```bash
# terminal 1: a validator with the program loaded
solana-test-validator --reset --bpf-program 6se56Bge1Gg9YD9yK7EUe8r2wmU9FJTWKkhseSKiMbQy target/deploy/otc_desk.so

# terminal 2: the Blink app
SOLANA_CLUSTER=localnet npm run dev

# terminal 3: create an offer (mints must exist and your wallet must hold token A)
SOLANA_CLUSTER=localnet npm run make-offer -- \
  --mint-a <MINT_A> --amount-a 500 --mint-b <MINT_B> --amount-b 1250 --partial --expires-in-hours 48
```

The CLI prints the offer page and its Action API URL. The offer page has an **Open as a Blink** button that opens it on [dial.to](https://dial.to).

### Deploy to devnet

```bash
solana config set --url devnet
anchor deploy --provider.cluster devnet
SOLANA_CLUSTER=devnet npm run dev    # or deploy app/ to Vercel with SOLANA_CLUSTER=devnet
```

Blinks only unfurl on X once your domain is registered with the [Dialect registry](https://dial.to/register).

## Security notes

- This code is **unaudited**. Don't use it with real funds without a review.
- Transfer-hook mints are not supported, because the program doesn't forward extra hook accounts in its CPIs.
- On a Token-2022 mint with transfer fees, both the maker's deposit and the taker's withdrawal pay the fee. The price applies to what reached the vault.

## License

MIT. The original work is © 2025 Mike MacCana, and the modifications are © 2026 Dobrin Dobrev. See [LICENSE.md](LICENSE.md).
