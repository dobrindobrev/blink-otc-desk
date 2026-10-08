import {
  address,
  createSolanaRpc,
  getBase58Decoder,
  isAddress,
  type Address,
  type Rpc,
  type SolanaRpcApi,
} from "@solana/kit";
import { decodeOffer, fetchMaybeOffer, findOfferPda, getOfferSize, OFFER_DISCRIMINATOR, OTC_DESK_PROGRAM_ADDRESS, type Offer } from "../generated";
import { RPC_URL } from "./config";

export type SolanaRpc = Rpc<SolanaRpcApi>;

export function getRpc(): SolanaRpc {
  return createSolanaRpc(RPC_URL);
}

export interface MintInfo {
  address: Address;
  decimals: number;
  /** SPL Token or Token-2022 — whichever program owns the mint */
  tokenProgram: Address;
}

export interface OfferDetails {
  address: Address;
  data: Offer;
  mintA: MintInfo;
  mintB: MintInfo;
}

// The mint layout is the same for both token programs: decimals is byte 44
const MINT_DECIMALS_OFFSET = 44;

export async function fetchMintInfo(rpc: SolanaRpc, mint: Address): Promise<MintInfo> {
  const { value } = await rpc.getAccountInfo(mint, { encoding: "base64" }).send();
  if (!value) throw new Error(`Mint ${mint} not found`);
  const data = Buffer.from(value.data[0], "base64");
  return { address: mint, decimals: data[MINT_DECIMALS_OFFSET], tokenProgram: value.owner };
}

async function withMints(rpc: SolanaRpc, offerAddress: Address, data: Offer): Promise<OfferDetails> {
  const [mintA, mintB] = await Promise.all([fetchMintInfo(rpc, data.tokenMintA), fetchMintInfo(rpc, data.tokenMintB)]);
  return { address: offerAddress, data, mintA, mintB };
}

/** Parses the `[maker, id]` route params; returns null if they're malformed. */
export function parseOfferParams(maker: string, id: string): { maker: Address; id: bigint } | null {
  if (!isAddress(maker) || !/^\d{1,20}$/.test(id)) return null;
  const parsed = BigInt(id);
  return parsed <= 0xffff_ffff_ffff_ffffn ? { maker: address(maker), id: parsed } : null;
}

export async function loadOffer(rpc: SolanaRpc, maker: Address, id: bigint): Promise<OfferDetails | null> {
  const [offerAddress] = await findOfferPda({ maker, id });
  const account = await fetchMaybeOffer(rpc, offerAddress);
  if (!account.exists) return null;
  return withMints(rpc, offerAddress, account.data);
}

/** All open offers, newest IDs first. */
export async function listOffers(rpc: SolanaRpc): Promise<OfferDetails[]> {
  const accounts = await rpc
    .getProgramAccounts(OTC_DESK_PROGRAM_ADDRESS, {
      encoding: "base64",
      filters: [
        { dataSize: BigInt(getOfferSize()) },
        {
          memcmp: {
            offset: 0n,
            bytes: getBase58Decoder().decode(OFFER_DISCRIMINATOR) as never,
            encoding: "base58",
          },
        },
      ],
    })
    .send();
  const offers = accounts.map(({ pubkey, account }) =>
    decodeOffer({
      address: pubkey,
      data: Buffer.from(account.data[0], "base64"),
      executable: account.executable,
      lamports: account.lamports,
      programAddress: account.owner,
      space: account.space,
    }),
  );
  const details = await Promise.all(offers.map((offer) => withMints(rpc, offer.address, offer.data)));
  return details.sort((a, b) => Number(b.data.id - a.data.id));
}

export function isExpired(offer: Offer, nowSeconds = Math.floor(Date.now() / 1000)): boolean {
  return offer.expiresAt !== 0n && BigInt(nowSeconds) >= offer.expiresAt;
}
