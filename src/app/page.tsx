import Link from "next/link";

export default function Home() {
  return (
    <>
      <h1>OnePass</h1>
      <p className="sub">One identity. Every door.</p>
      <div className="grid">
        <div className="card">
          <h2>1. University</h2>
          <p>Signs a student credential and hands it to the student.</p>
          <Link className="button" href="/issuer">Get a credential</Link>
        </div>
        <div className="card">
          <h2>2. Wallet</h2>
          <p>Holds your credentials. You choose what each company sees.</p>
          <Link className="button" href="/wallet">Open wallet</Link>
        </div>
        <div className="card">
          <h2>3. Company</h2>
          <p>Asks for proof, then checks it against the Stellar registry.</p>
          <Link className="button" href="/verifier">Verify a student</Link>
        </div>
      </div>
    </>
  );
}
