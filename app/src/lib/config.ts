// Solana Actions identify chains by genesis hash (CAIP-2)
const BLOCKCHAIN_IDS = {
  mainnet: "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp",
  devnet: "solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1",
  localnet: "solana:localnet",
} as const;

export type Cluster = keyof typeof BLOCKCHAIN_IDS;

export const CLUSTER = (process.env.SOLANA_CLUSTER ?? "devnet") as Cluster;
export const RPC_URL =
  process.env.SOLANA_RPC_URL ?? (CLUSTER === "localnet" ? "http://127.0.0.1:8899" : `https://api.${CLUSTER}.solana.com`);
export const BLOCKCHAIN_ID = BLOCKCHAIN_IDS[CLUSTER];
