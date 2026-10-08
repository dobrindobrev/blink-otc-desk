import { headers } from "next/headers";
import { notFound } from "next/navigation";
import { formatAmount, tokenBFor } from "@/lib/amounts";
import { CLUSTER } from "@/lib/config";
import { getRpc, isExpired, loadOffer, parseOfferParams } from "@/lib/offers";

export const dynamic = "force-dynamic";

export default async function OfferPage({ params }: { params: Promise<{ maker: string; id: string }> }) {
  const { maker, id } = await params;
  const parsed = parseOfferParams(maker, id);
  if (!parsed) notFound();

  const offer = await loadOffer(getRpc(), parsed.maker, parsed.id);
  if (!offer) {
    return (
      <>
        <h1>Offer closed</h1>
        <p className="lead">This offer has been filled or cancelled.</p>
      </>
    );
  }

  const { data, mintA, mintB } = offer;
  const host = (await headers()).get("host") ?? "localhost:3000";
  const protocol = host.startsWith("localhost") ? "http" : "https";
  const actionUrl = `${protocol}://${host}/api/actions/offer/${maker}/${id}`;
  const blinkUrl = `https://dial.to/?action=${encodeURIComponent(`solana-action:${actionUrl}`)}&cluster=${CLUSTER}`;
  const price = tokenBFor(data.tokenARemaining, data);

  return (
    <>
      <h1>
        {formatAmount(data.tokenARemaining, mintA.decimals)} for {formatAmount(price, mintB.decimals)}
      </h1>
      <p className="lead">Offer #{data.id.toString()}, settled on-chain by the OTC Desk program.</p>
      <dl>
        <dt>Selling</dt>
        <dd className="mono">{mintA.address}</dd>
        <dt>For</dt>
        <dd className="mono">{mintB.address}</dd>
        <dt>Maker</dt>
        <dd className="mono">{data.maker}</dd>
        <dt>Filled</dt>
        <dd>
          {formatAmount(data.tokenAOfferedAmount - data.tokenARemaining, mintA.decimals)} of{" "}
          {formatAmount(data.tokenAOfferedAmount, mintA.decimals)}
        </dd>
        <dt>Fills</dt>
        <dd>{data.allowPartialFills ? "Partial fills allowed" : "All or nothing"}</dd>
        <dt>Expires</dt>
        <dd>
          {data.expiresAt === 0n ? "Never" : new Date(Number(data.expiresAt) * 1000).toUTCString()}
          {isExpired(data) && " (expired)"}
        </dd>
        <dt>Offer account</dt>
        <dd className="mono">{offer.address}</dd>
      </dl>
      <a className="button" href={blinkUrl} target="_blank" rel="noreferrer">
        Open as a Blink
      </a>
    </>
  );
}
