import { NextResponse } from "next/server";
import { randomUUID } from "crypto";
import { requests } from "@/lib/store";
import type { VerificationRequest } from "@/lib/types";

// A company asks for specific pieces of information.
export async function POST(req: Request) {
  const { verifier, requiredClaims } = await req.json();
  if (!verifier || !Array.isArray(requiredClaims) || requiredClaims.length === 0) {
    return NextResponse.json({ error: "verifier and requiredClaims are required" }, { status: 400 });
  }
  const request: VerificationRequest = {
    id: randomUUID(),
    verifier,
    requiredClaims,
    status: "pending",
  };
  requests.set(request.id, request);
  return NextResponse.json(request);
}
