import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: "OnePass",
  description: "One identity. Every door.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <nav>
          <Link href="/" className="brand">OnePass</Link>
          <Link href="/issuer">University</Link>
          <Link href="/wallet">Wallet</Link>
          <Link href="/verifier">Company</Link>
        </nav>
        <main>{children}</main>
      </body>
    </html>
  );
}
