// Creates an OTC offer from a local keypair and prints its Blink link.
//
//   npm run make-offer -- --mint-a <MINT> --amount-a 100 --mint-b <MINT> --amount-b 250 \
//     [--partial] [--expires-in-hours 24] [--keypair ~/.config/solana/id.json] [--app-url http://localhost:3000]
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { parseArgs } from "node:util";
import { address, createKeyPairSignerFromBytes } from "@solana/kit";
import { getMakeOfferInstructionAsync } from "../src/generated";
import { parseAmount } from "../src/lib/amounts";
import { CLUSTER, RPC_URL } from "../src/lib/config";
import { fetchMintInfo, findOffer, getRpc } from "./shared";
import { sendInstructions } from "../src/lib/send";

const { values } = parseArgs({
  options: {
    "mint-a": { type: "string" },
    "amount-a": { type: "string" },
    "mint-b": { type: "string" },
    "amount-b": { type: "string" },
    partial: { type: "boolean", default: false },
    "expires-in-hours": { type: "string" },
    keypair: { type: "string", default: `${homedir()}/.config/solana/id.json` },
    id: { type: "string" },
    "app-url": { type: "string", default: "http://localhost:3000" },
  },
});

for (const required of ["mint-a", "amount-a", "mint-b", "amount-b"] as const) {
  if (!values[required]) throw new Error(`--${required} is required`);
}

const rpc = getRpc();
const maker = await createKeyPairSignerFromBytes(new Uint8Array(JSON.parse(readFileSync(values.keypair!, "utf8"))));
const [mintA, mintB] = await Promise.all([
  fetchMintInfo(rpc, address(values["mint-a"]!)),
  fetchMintInfo(rpc, address(values["mint-b"]!)),
]);
const amountA = parseAmount(values["amount-a"]!, mintA.decimals);
const amountB = parseAmount(values["amount-b"]!, mintB.decimals);
if (!amountA || !amountB) throw new Error("Amounts must be positive decimals within each mint's precision");

const id = values.id ? BigInt(values.id) : BigInt(Date.now());
const expiresAt = values["expires-in-hours"]
  ? BigInt(Math.floor(Date.now() / 1000 + Number(values["expires-in-hours"]) * 3600))
  : 0n;

const instruction = await getMakeOfferInstructionAsync({
  tokenProgramA: mintA.tokenProgram,
  tokenProgramB: mintB.tokenProgram,
  maker,
  tokenMintA: mintA.address,
  tokenMintB: mintB.address,
  id,
  tokenAOfferedAmount: amountA,
  tokenBWantedAmount: amountB,
  allowPartialFills: values.partial!,
  expiresAt,
});
const signature = await sendInstructions(rpc, maker, [instruction]);

console.log(`Offer #${id} created on ${CLUSTER} (${RPC_URL})`);
console.log(`  offer account: ${await findOffer(maker.address, id)}`);
console.log(`  transaction:   ${signature}`);
console.log(`  page:          ${values["app-url"]}/offer/${maker.address}/${id}`);
console.log(`  action API:    ${values["app-url"]}/api/actions/offer/${maker.address}/${id}`);
