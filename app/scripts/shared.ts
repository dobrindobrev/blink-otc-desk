import type { Address } from "@solana/kit";
import { findOfferPda } from "../src/generated";

export { fetchMintInfo, getRpc } from "../src/lib/offers";

export async function findOffer(maker: Address, id: bigint): Promise<Address> {
  const [offer] = await findOfferPda({ maker, id });
  return offer;
}
