import { describe, expect, it } from "vitest";
import { formatAmount, parseAmount, tokenBFor } from "../src/lib/amounts";

describe("formatAmount", () => {
  it("formats whole and fractional amounts", () => {
    expect(formatAmount(1_500_000n, 6)).toBe("1.5");
    expect(formatAmount(1_000_000n, 6)).toBe("1");
    expect(formatAmount(1n, 6)).toBe("0.000001");
    expect(formatAmount(0n, 0)).toBe("0");
  });
});

describe("parseAmount", () => {
  it("parses decimals into base units", () => {
    expect(parseAmount("1.5", 6)).toBe(1_500_000n);
    expect(parseAmount("42", 6)).toBe(42_000_000n);
    expect(parseAmount("0.000001", 6)).toBe(1n);
  });

  it("rejects invalid input and excess precision", () => {
    expect(parseAmount("1.0000001", 6)).toBeNull();
    expect(parseAmount("-1", 6)).toBeNull();
    expect(parseAmount("abc", 6)).toBeNull();
    expect(parseAmount("1e3", 6)).toBeNull();
  });
});

describe("tokenBFor (mirrors the on-chain pricing)", () => {
  const offer = { tokenAOfferedAmount: 3n, tokenBWantedAmount: 10n, tokenARemaining: 3n, tokenBReceived: 0n };

  it("rounds partial fills up for the maker", () => {
    expect(tokenBFor(1n, offer)).toBe(4n);
  });

  it("charges the remainder on the final fill", () => {
    expect(tokenBFor(1n, { ...offer, tokenARemaining: 1n, tokenBReceived: 8n })).toBe(2n);
  });

  it("charges the full price for a full fill", () => {
    expect(tokenBFor(3n, offer)).toBe(10n);
  });
});
