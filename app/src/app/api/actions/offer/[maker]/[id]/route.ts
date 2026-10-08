import { isAddress } from "@solana/kit";
import {
  actionError,
  actionJson,
  preflight,
  type ActionGetResponse,
  type ActionPostRequest,
  type ActionPostResponse,
} from "@/lib/actions";
import { formatAmount, parseAmount, shortAddress, tokenBFor } from "@/lib/amounts";
import { getRpc, isExpired, loadOffer, parseOfferParams, type OfferDetails } from "@/lib/offers";
import { buildTakeTransaction } from "@/lib/take";

type Params = { params: Promise<{ maker: string; id: string }> };

export const dynamic = "force-dynamic";

function describe(offer: OfferDetails) {
  const { data, mintA, mintB } = offer;
  const tokenA = shortAddress(mintA.address);
  const tokenB = shortAddress(mintB.address);
  const remaining = formatAmount(data.tokenARemaining, mintA.decimals);
  const price = tokenBFor(data.tokenARemaining, data);
  return { tokenA, tokenB, remaining, price: formatAmount(price, mintB.decimals) };
}

export async function GET(request: Request, { params }: Params) {
  const { maker, id } = await params;
  const parsed = parseOfferParams(maker, id);
  if (!parsed) return actionError("Invalid offer link", 404);

  const origin = new URL(request.url).origin;
  const icon = `${origin}/otc-desk.svg`;
  const offer = await loadOffer(getRpc(), parsed.maker, parsed.id);
  if (!offer) {
    return actionJson({
      type: "action",
      icon,
      title: "Offer closed",
      description: "This offer has been filled or cancelled.",
      label: "Closed",
      disabled: true,
    } satisfies ActionGetResponse);
  }

  const { tokenA, tokenB, remaining, price } = describe(offer);
  const expiresAt = offer.data.expiresAt;
  const expiry = expiresAt === 0n ? "Never expires." : `Expires ${new Date(Number(expiresAt) * 1000).toUTCString()}.`;
  const base = `/api/actions/offer/${maker}/${id}`;
  const actions: NonNullable<ActionGetResponse["links"]>["actions"] = [
    { type: "transaction", label: `Buy all for ${price} ${tokenB}`, href: `${base}?amount=all` },
  ];
  if (offer.data.allowPartialFills) {
    actions.push({
      type: "transaction",
      label: "Buy",
      href: `${base}?amount={amount}`,
      parameters: [{ name: "amount", label: `Amount of ${tokenA} to buy (max ${remaining})`, type: "number", required: true }],
    });
  }

  const expired = isExpired(offer.data);
  return actionJson({
    type: "action",
    icon,
    title: `${remaining} ${tokenA} for ${price} ${tokenB}`,
    description: `Peer-to-peer OTC offer from ${shortAddress(offer.data.maker)}. ${
      offer.data.allowPartialFills ? "Partial fills allowed." : "All or nothing."
    } ${expiry} Settled atomically by the OTC Desk program — no custody, no fees.`,
    label: "Buy",
    disabled: expired,
    error: expired ? { message: "This offer has expired." } : undefined,
    links: { actions },
  } satisfies ActionGetResponse);
}

export async function POST(request: Request, { params }: Params) {
  const { maker, id } = await params;
  const parsed = parseOfferParams(maker, id);
  if (!parsed) return actionError("Invalid offer link", 404);

  let body: ActionPostRequest;
  try {
    body = await request.json();
  } catch {
    return actionError("Request body must be JSON with an `account` field");
  }
  if (typeof body.account !== "string" || !isAddress(body.account)) {
    return actionError("Invalid `account`: expected a base58 Solana address");
  }

  const rpc = getRpc();
  const offer = await loadOffer(rpc, parsed.maker, parsed.id);
  if (!offer) return actionError("This offer has been filled or cancelled", 404);
  if (isExpired(offer.data)) return actionError("This offer has expired");
  if (body.account === offer.data.maker) return actionError("You can't take your own offer");

  const amountParam = new URL(request.url).searchParams.get("amount") ?? "all";
  const tokenAAmount =
    amountParam === "all" ? offer.data.tokenARemaining : parseAmount(amountParam, offer.mintA.decimals);
  if (tokenAAmount === null || tokenAAmount <= 0n) return actionError("Enter a positive amount");
  if (tokenAAmount > offer.data.tokenARemaining) {
    return actionError(`Only ${formatAmount(offer.data.tokenARemaining, offer.mintA.decimals)} left`);
  }
  if (!offer.data.allowPartialFills && tokenAAmount !== offer.data.tokenARemaining) {
    return actionError("This offer must be bought in full");
  }

  const { transaction, tokenBAmount } = await buildTakeTransaction(rpc, offer, body.account, tokenAAmount);
  const { tokenA, tokenB } = describe(offer);
  return actionJson({
    type: "transaction",
    transaction,
    message: `Buying ${formatAmount(tokenAAmount, offer.mintA.decimals)} ${tokenA} for ${formatAmount(
      tokenBAmount,
      offer.mintB.decimals,
    )} ${tokenB}`,
  } satisfies ActionPostResponse);
}

export const OPTIONS = preflight;
