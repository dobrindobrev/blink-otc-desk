// End-to-end: a real local validator running the program, the Blink API
// route handlers, and a "wallet" that signs whatever the Blink returns.
//
//   anchor build && npm run test:e2e -w app
import { spawn, type ChildProcess } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  generateKeyPairSigner,
  getBase64EncodedWireTransaction,
  getTransactionDecoder,
  signTransaction,
  type Address,
  type KeyPairSigner,
} from "@solana/kit";
import { getCreateAccountInstruction } from "@solana-program/system";
import {
  fetchToken,
  findAssociatedTokenPda,
  getCreateAssociatedTokenIdempotentInstructionAsync,
  getInitializeMint2Instruction,
  getMintSize,
  getMintToInstruction,
  TOKEN_PROGRAM_ADDRESS,
} from "@solana-program/token";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { getMakeOfferInstructionAsync, OTC_DESK_PROGRAM_ADDRESS } from "../src/generated";
import { getRpc, loadOffer, type SolanaRpc } from "../src/lib/offers";
import { sendInstructions, sendWireTransaction, waitForConfirmation } from "../src/lib/send";
import { GET, POST } from "../src/app/api/actions/offer/[maker]/[id]/route";

const DECIMALS = 6;
const TOKEN = 10n ** BigInt(DECIMALS);
const PROGRAM_SO = new URL("../../target/deploy/otc_desk.so", import.meta.url).pathname;

let validator: ChildProcess;
let rpc: SolanaRpc;

async function waitForValidator() {
  for (let attempt = 0; attempt < 120; attempt++) {
    try {
      await rpc.getLatestBlockhash().send();
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
  }
  throw new Error("solana-test-validator did not start");
}

async function fundedSigner(): Promise<KeyPairSigner> {
  const signer = await generateKeyPairSigner();
  const signature = await rpc.requestAirdrop(signer.address, 10_000_000_000n as never).send();
  await waitForConfirmation(rpc, signature);
  return signer;
}

async function createMint(authority: KeyPairSigner): Promise<Address> {
  const mint = await generateKeyPairSigner();
  const space = BigInt(getMintSize());
  const rent = await rpc.getMinimumBalanceForRentExemption(space).send();
  await sendInstructions(rpc, authority, [
    getCreateAccountInstruction({ payer: authority, newAccount: mint, lamports: rent, space, programAddress: TOKEN_PROGRAM_ADDRESS }),
    getInitializeMint2Instruction({ mint: mint.address, decimals: DECIMALS, mintAuthority: authority.address }),
  ]);
  return mint.address;
}

async function ata(owner: Address, mint: Address): Promise<Address> {
  const [address] = await findAssociatedTokenPda({ owner, mint, tokenProgram: TOKEN_PROGRAM_ADDRESS });
  return address;
}

async function mintTo(authority: KeyPairSigner, mint: Address, owner: Address, amount: bigint) {
  await sendInstructions(rpc, authority, [
    await getCreateAssociatedTokenIdempotentInstructionAsync({ payer: authority, owner, mint }),
    getMintToInstruction({ mint, token: await ata(owner, mint), mintAuthority: authority, amount }),
  ]);
}

async function balance(owner: Address, mint: Address): Promise<bigint> {
  return (await fetchToken(rpc, await ata(owner, mint))).data.amount;
}

function params(maker: Address, id: bigint) {
  return { params: Promise.resolve({ maker, id: id.toString() }) };
}

function actionRequest(maker: Address, id: bigint, query = "", body?: object) {
  return new Request(`http://localhost:3000/api/actions/offer/${maker}/${id}${query}`, {
    method: body ? "POST" : "GET",
    body: body ? JSON.stringify(body) : undefined,
    headers: { "content-type": "application/json" },
  });
}

/** What a wallet does with a Blink's POST response: sign and send. */
async function signAndSend(base64: string, signer: KeyPairSigner) {
  const transaction = getTransactionDecoder().decode(Buffer.from(base64, "base64"));
  const signed = await signTransaction([signer.keyPair], transaction);
  return sendWireTransaction(rpc, getBase64EncodedWireTransaction(signed));
}

describe.runIf(process.env.E2E)("Blink against a local validator", () => {
  let maker: KeyPairSigner;
  let taker: KeyPairSigner;
  let mintA: Address;
  let mintB: Address;
  const id = 7n;

  beforeAll(async () => {
    const ledger = mkdtempSync(join(tmpdir(), "otc-desk-ledger-"));
    validator = spawn(
      "solana-test-validator",
      ["--reset", "--quiet", "--ledger", ledger, "--bpf-program", OTC_DESK_PROGRAM_ADDRESS, PROGRAM_SO],
      { stdio: "ignore" },
    );
    rpc = getRpc();
    await waitForValidator();

    [maker, taker] = await Promise.all([fundedSigner(), fundedSigner()]);
    mintA = await createMint(maker);
    mintB = await createMint(maker);
    await mintTo(maker, mintA, maker.address, 100n * TOKEN);
    await mintTo(maker, mintB, taker.address, 1_000n * TOKEN);

    await sendInstructions(rpc, maker, [
      await getMakeOfferInstructionAsync({
        tokenProgramA: TOKEN_PROGRAM_ADDRESS,
        tokenProgramB: TOKEN_PROGRAM_ADDRESS,
        maker,
        tokenMintA: mintA,
        tokenMintB: mintB,
        id,
        tokenAOfferedAmount: 100n * TOKEN,
        tokenBWantedAmount: 250n * TOKEN,
        allowPartialFills: true,
        expiresAt: 0n,
      }),
    ]);
  });

  afterAll(() => {
    validator?.kill();
  });

  it("serves Action metadata for an open offer", async () => {
    const response = await GET(actionRequest(maker.address, id), params(maker.address, id));
    expect(response.status).toBe(200);
    expect(response.headers.get("access-control-allow-origin")).toBe("*");
    expect(response.headers.get("x-action-version")).toBe("2.4");

    const action = await response.json();
    expect(action.type).toBe("action");
    expect(action.title).toMatch(/^100 .* for 250 /);
    expect(action.links.actions).toHaveLength(2);
    expect(action.links.actions[1].parameters[0].name).toBe("amount");
  });

  it("returns a transaction that fills part of the offer when signed", async () => {
    const response = await POST(actionRequest(maker.address, id, "?amount=40", { account: taker.address }), params(maker.address, id));
    expect(response.status).toBe(200);
    const { transaction, message } = await response.json();
    expect(message).toMatch(/^Buying 40 .* for 100 /);

    await signAndSend(transaction, taker);

    expect(await balance(taker.address, mintA)).toBe(40n * TOKEN);
    expect(await balance(taker.address, mintB)).toBe(900n * TOKEN);
    expect(await balance(maker.address, mintB)).toBe(100n * TOKEN);
  });

  it("rejects amounts the offer can't fill", async () => {
    const response = await POST(actionRequest(maker.address, id, "?amount=61", { account: taker.address }), params(maker.address, id));
    expect(response.status).toBe(400);
    expect((await response.json()).message).toBe("Only 60 left");
  });

  it("buys the rest, closing the offer", async () => {
    const response = await POST(actionRequest(maker.address, id, "?amount=all", { account: taker.address }), params(maker.address, id));
    await signAndSend((await response.json()).transaction, taker);

    expect(await balance(taker.address, mintA)).toBe(100n * TOKEN);
    expect(await balance(maker.address, mintB)).toBe(250n * TOKEN);
    expect(await loadOffer(rpc, maker.address, id)).toBeNull();

    const closed = await (await GET(actionRequest(maker.address, id), params(maker.address, id))).json();
    expect(closed.disabled).toBe(true);
  });
});
