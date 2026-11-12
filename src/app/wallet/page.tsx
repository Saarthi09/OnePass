"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import type { Presentation, VerificationRequest, WalletEntry } from "@/lib/types";
import { CLAIM_LABELS, loadWallet, saveWallet, show } from "@/lib/wallet";

export default function WalletPage() {
  const [entries, setEntries] = useState<WalletEntry[]>([]);
  const [request, setRequest] = useState<VerificationRequest | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState("");

  useEffect(() => {
    setEntries(loadWallet());
    const id = new URLSearchParams(window.location.search).get("request");
    if (!id) return;
    fetch(`/api/requests/${id}`)
      .then((r) => (r.ok ? r.json() : Promise.reject()))
      .then((req: VerificationRequest) => {
        setRequest(req);
        setSelected(new Set(req.requiredClaims));
      })
      .catch(() => setError("That request no longer exists."));
  }, []);

  // First credential that can answer everything the company asked for.
  const match = request
    ? entries.find((e) => request.requiredClaims.every((c) => c in e.disclosures))
    : undefined;

  function toggle(claim: string) {
    const next = new Set(selected);
    if (next.has(claim)) next.delete(claim);
    else next.add(claim);
    setSelected(next);
  }

  async function approve() {
    if (!request || !match) return;
    const presentation: Presentation = {
      credential: match.credential,
      revealed: Object.fromEntries([...selected].map((c) => [c, match.disclosures[c]])),
    };
    const res = await fetch("/api/present", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ requestId: request.id, presentation }),
    });
    const data = await res.json();
    if (!res.ok) return setError(data.error ?? "Could not share");
    setRequest(data);
  }

  function remove(index: number) {
    const next = entries.filter((_, i) => i !== index);
    saveWallet(next);
    setEntries(next);
  }

  return (
    <>
      <h1>Your wallet</h1>
      <p className="sub">Stored only in this browser. You approve every share.</p>
      {error && <p className="bad">{error}</p>}

      {request?.status === "pending" && (
        <div className="card">
          <h2>{request.verifier} is asking for information</h2>
          {!match ? (
            <>
              <p>You have no credential that can answer this request.</p>
              <Link className="button" href="/issuer">Get a student credential</Link>
            </>
          ) : (
            <>
              {Object.entries(match.disclosures).map(([claim, d]) => {
                const required = request.requiredClaims.includes(claim);
                return (
                  <label className="check" key={claim} style={{ margin: 0 }}>
                    <input
                      type="checkbox"
                      checked={selected.has(claim)}
                      disabled={required}
                      onChange={() => toggle(claim)}
                    />
                    <span style={{ color: "var(--ink)" }}>
                      {CLAIM_LABELS[claim] ?? claim}: <strong>{show(d.value)}</strong>
                    </span>
                    <span className="tag">{required ? "requested" : "optional"}</span>
                  </label>
                );
              })}
              <button onClick={approve}>Share {selected.size} item{selected.size === 1 ? "" : "s"}</button>
            </>
          )}
        </div>
      )}

      {request && request.status !== "pending" && (
        <div className="card">
          <h2 className={request.status === "verified" ? "ok" : "bad"}>
            {request.status === "verified"
              ? `Shared with ${request.verifier}`
              : `${request.verifier} could not verify this`}
          </h2>
          <p>You can close this tab and return to {request.verifier}.</p>
        </div>
      )}

      <h2 style={{ marginTop: 24 }}>Credentials</h2>
      {entries.length === 0 && (
        <div className="card">
          <p>No credentials yet.</p>
          <Link className="button" href="/issuer">Get a student credential</Link>
        </div>
      )}
      {entries.map((e, i) => (
        <div className="card" key={e.credential.signature}>
          <h2>{e.credential.issuerName} <span className="tag">{e.credential.type}</span></h2>
          {Object.entries(e.disclosures).map(([claim, d]) => (
            <div className="row" key={claim}><span>{CLAIM_LABELS[claim] ?? claim}</span><span>{show(d.value)}</span></div>
          ))}
          <button className="secondary" onClick={() => remove(i)}>Remove</button>
        </div>
      ))}
    </>
  );
}
