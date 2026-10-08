import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "OTC Desk",
  description: "Peer-to-peer OTC token offers on Solana, shareable as Blinks.",
  icons: { icon: "/otc-desk.svg" },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <header className="header">
          <a href="/" className="brand">
            <img src="/otc-desk.svg" alt="" width={28} height={28} />
            OTC Desk
          </a>
          <span className="cluster">{process.env.SOLANA_CLUSTER ?? "devnet"}</span>
        </header>
        <main className="main">{children}</main>
      </body>
    </html>
  );
}
