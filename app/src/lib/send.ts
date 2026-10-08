// Small helpers for scripts and tests that sign and send with a local keypair.
import {
  appendTransactionMessageInstructions,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  getSignatureFromTransaction,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  type Instruction,
  type Signature,
  type TransactionSigner,
} from "@solana/kit";
import type { SolanaRpc } from "./offers";

export async function waitForConfirmation(rpc: SolanaRpc, signature: Signature, timeoutMs = 30_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const { value } = await rpc.getSignatureStatuses([signature]).send();
    const status = value[0];
    if (status?.err) throw new Error(`Transaction ${signature} failed: ${JSON.stringify(status.err)}`);
    if (status?.confirmationStatus === "confirmed" || status?.confirmationStatus === "finalized") return;
    await new Promise((resolve) => setTimeout(resolve, 400));
  }
  throw new Error(`Timed out waiting for ${signature}`);
}

export async function sendWireTransaction(rpc: SolanaRpc, base64Transaction: string): Promise<Signature> {
  const signature = await rpc
    .sendTransaction(base64Transaction as never, { encoding: "base64", preflightCommitment: "confirmed" })
    .send();
  await waitForConfirmation(rpc, signature);
  return signature;
}

export async function sendInstructions(
  rpc: SolanaRpc,
  feePayer: TransactionSigner,
  instructions: Instruction[],
): Promise<Signature> {
  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
  const message = pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(feePayer, m),
    (m) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
    (m) => appendTransactionMessageInstructions(instructions, m),
  );
  const signed = await signTransactionMessageWithSigners(message);
  await sendWireTransaction(rpc, getBase64EncodedWireTransaction(signed));
  return getSignatureFromTransaction(signed);
}
