import { formatAmount, shortAddress, tokenBFor } from "@/lib/amounts";
import { getRpc, isExpired, listOffers } from "@/lib/offers";

export const dynamic = "force-dynamic";

export default async function Home() {
  const offers = await listOffers(getRpc());

  return (
    <>
      <h1>Open offers</h1>
      <p className="lead">
        Every offer is a Blink: share its link on X, Discord or Telegram and anyone can fill it from their wallet.
      </p>
      {offers.length === 0 ? (
        <div className="empty">
          No open offers yet. Create one with <code>npm run make-offer</code>.
        </div>
      ) : (
        <div className="offers">
          {offers.map(({ address, data, mintA, mintB }) => (
            <a key={address} className="card" href={`/offer/${data.maker}/${data.id}`}>
              <span className="pair">
                {formatAmount(data.tokenARemaining, mintA.decimals)} {shortAddress(mintA.address)} →{" "}
                {formatAmount(tokenBFor(data.tokenARemaining, data), mintB.decimals)} {shortAddress(mintB.address)}
              </span>
              <span className={`tag${isExpired(data) ? " expired" : ""}`}>
                {isExpired(data) ? "expired" : data.allowPartialFills ? "partial fills" : "all or nothing"}
              </span>
              <span className="meta">
                by {shortAddress(data.maker)} · #{data.id.toString()}
              </span>
            </a>
          ))}
        </div>
      )}
    </>
  );
}
