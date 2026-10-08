/** Formats base units as a decimal string, e.g. (1_500_000n, 6) -> "1.5". */
export function formatAmount(amount: bigint, decimals: number): string {
  const base = 10n ** BigInt(decimals);
  const whole = amount / base;
  const fraction = (amount % base).toString().padStart(decimals, "0").replace(/0+$/, "");
  return fraction ? `${whole}.${fraction}` : whole.toString();
}

/** Parses a decimal string into base units. Returns null for invalid input or too many decimals. */
export function parseAmount(value: string, decimals: number): bigint | null {
  const match = /^(\d+)(?:\.(\d+))?$/.exec(value.trim());
  if (!match) return null;
  const [, whole, fraction = ""] = match;
  if (fraction.length > decimals) return null;
  return BigInt(whole) * 10n ** BigInt(decimals) + BigInt(fraction.padEnd(decimals, "0") || "0");
}

/** Token B owed for a fill, mirroring `pricing::token_b_for` in the program. */
export function tokenBFor(
  tokenAAmount: bigint,
  offer: { tokenARemaining: bigint; tokenAOfferedAmount: bigint; tokenBWantedAmount: bigint; tokenBReceived: bigint },
): bigint {
  if (tokenAAmount === offer.tokenARemaining) {
    const rest = offer.tokenBWantedAmount - offer.tokenBReceived;
    return rest > 0n ? rest : 0n;
  }
  const numerator = tokenAAmount * offer.tokenBWantedAmount;
  return (numerator + offer.tokenAOfferedAmount - 1n) / offer.tokenAOfferedAmount;
}

export function shortAddress(address: string): string {
  return `${address.slice(0, 4)}…${address.slice(-4)}`;
}
