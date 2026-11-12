"use client";

import { useEffect, useState } from "react";
import type { VerificationRequest } from "@/lib/types";
import { CLAIM_LABELS, show } from "@/lib/wallet";

const REQUIRED = ["isStudent", "university"];

export default function VerifierPage() {
  const [request, setRequest] = useState<VerificationRequest | null>(null);

  async function start() {
    const res = await fetch("/api/requests", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ verifier: "Puma", requiredClaims: REQUIRED }),
    });
    setRequest(await res.json());
  }

  // Poll until the user answers in their wallet.
  useEffect(() => {
    if (!request || request.status !== "pending") return;
    const timer = setInterval(async () => {
      const res = await fetch(`/api/requests/${request.id}`);
      if (res.ok) setRequest(await res.json());
    }, 1500);
    return () => clearInterval(timer);
  }, [request]);

  return (
    <>
      <h1>Puma</h1>
      <p className="sub">Demo company: 20% student discount, verified with OnePass.</p>

      {!request && (
        <div className="card">
          <h2>Student discount</h2>
          <p>We only ask for two things: that you are a student, and where.</p>
          <button onClick={start}>Verify with OnePass</button>
        </div>
      )}

      {request?.status === "pending" && (
        <div className="card">
          <h2>Waiting for your approval…</h2>
          <p>Approve the request in your OnePass wallet.</p>
          <a className="button" href={`/wallet?request=${request.id}`} target="_blank" rel="noreferrer">
            Open wallet
          </a>
        </div>
      )}

      {request?.result && (
        <div className="card">
          <h2 className={request.result.ok ? "ok" : "bad"}>
            {request.result.ok ? "Verified: discount applied" : "Verification failed"}
          </h2>
          {request.result.checks.map((c) => (
            <div className="check" key={c.label}>
              <span className={c.passed ? "ok" : "bad"}>{c.passed ? "✓" : "✗"}</span>
              <span>{c.label}{c.detail ? `: ${c.detail}` : ""}</span>
            </div>
          ))}
          {request.result.ok && (
            <>
              <h2 style={{ marginTop: 20 }}>What Puma received</h2>
              {Object.entries(request.result.claims).map(([k, v]) => (
                <div className="row" key={k}><span>{CLAIM_LABELS[k] ?? k}</span><span>{show(v)}</span></div>
              ))}
            </>
          )}
          <button className="secondary" onClick={() => setRequest(null)}>Start over</button>
        </div>
      )}
    </>
  );
}
