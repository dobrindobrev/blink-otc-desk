import {
  appendTransactionMessageInstructions,
  compileTransaction,
  createNoopSigner,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  type Address,
} from "@solana/kit";
import { getTakeOfferInstructionAsync } from "../generated";
import { tokenBFor } from "./amounts";
import type { OfferDetails, SolanaRpc } from "./offers";

/**
 * Builds an unsigned take-offer transaction for `taker` to sign in their
 * wallet. The price guard is set to exactly the quoted price, so the
 * transaction fails rather than overpays if the offer changes before it lands.
 */
export async function buildTakeTransaction(
  rpc: SolanaRpc,
  offer: OfferDetails,
  taker: Address,
  tokenAAmount: bigint,
): Promise<{ transaction: string; tokenBAmount: bigint }> {
  const tokenBAmount = tokenBFor(tokenAAmount, offer.data);
  // The wallet signs later; a noop signer just marks the taker as a required signer
  const takerSigner = createNoopSigner(taker);

  const instruction = await getTakeOfferInstructionAsync({
    tokenProgramA: offer.mintA.tokenProgram,
    tokenProgramB: offer.mintB.tokenProgram,
    taker: takerSigner,
    maker: offer.data.maker,
    tokenMintA: offer.mintA.address,
    tokenMintB: offer.mintB.address,
    offer: offer.address,
    tokenAAmount,
    maxTokenBAmount: tokenBAmount,
  });

  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(takerSigner, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
    (m) => appendTransactionMessageInstructions([instruction], m),
  );
  return { transaction: getBase64EncodedWireTransaction(compileTransaction(message)), tokenBAmount };
}
