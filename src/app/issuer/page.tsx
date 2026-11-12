"use client";

import Link from "next/link";
import { useState } from "react";
import type { WalletEntry } from "@/lib/types";
import { loadWallet, saveWallet } from "@/lib/wallet";

export default function IssuerPage() {
  const [name, setName] = useState("");
  const [studentId, setStudentId] = useState("");
  const [birthDate, setBirthDate] = useState("");
  const [issued, setIssued] = useState<WalletEntry | null>(null);
  const [error, setError] = useState("");

  async function issue(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    const res = await fetch("/api/issue", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name, studentId, birthDate }),
    });
    const data = await res.json();
    if (!res.ok) return setError(data.error ?? "Could not issue credential");
    saveWallet([...loadWallet(), data]);
    setIssued(data);
  }

  return (
    <>
      <h1>University of Waterloo</h1>
      <p className="sub">Demo issuer: signs a student credential for your wallet.</p>

      {!issued ? (
        <form className="card" onSubmit={issue}>
          <h2>Request your student credential</h2>
          <label htmlFor="name">Full name</label>
          <input id="name" type="text" value={name} onChange={(e) => setName(e.target.value)} required />
          <label htmlFor="sid">Student ID</label>
          <input id="sid" type="text" value={studentId} onChange={(e) => setStudentId(e.target.value)} required />
          <label htmlFor="dob">Date of birth</label>
          <input id="dob" type="date" value={birthDate} onChange={(e) => setBirthDate(e.target.value)} required />
          {error && <p className="bad">{error}</p>}
          <button type="submit">Issue credential</button>
        </form>
      ) : (
        <div className="card">
          <h2 className="ok">Credential added to your wallet</h2>
          <div className="row"><span>Signed by</span><code>{issued.credential.issuer}</code></div>
          <div className="row"><span>Signature</span><code>{issued.credential.signature.slice(0, 44)}…</code></div>
          <Link className="button" href="/wallet">Open wallet</Link>
        </div>
      )}
    </>
  );
}
