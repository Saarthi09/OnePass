import { NextResponse } from "next/server";
import { requests } from "@/lib/store";
import { verifyPresentation } from "@/lib/verify";

// The user's wallet sends what they approved; the company verifies it.
export async function POST(req: Request) {
  const { requestId, presentation } = await req.json();
  const request = requests.get(requestId);
  if (!request) return NextResponse.json({ error: "Request not found" }, { status: 404 });
  if (request.status !== "pending") {
    return NextResponse.json({ error: "Request already answered" }, { status: 409 });
  }
  if (!presentation?.credential || !presentation?.revealed) {
    return NextResponse.json({ error: "Invalid presentation" }, { status: 400 });
  }
  const result = await verifyPresentation(presentation, request.requiredClaims);
  request.status = result.ok ? "verified" : "rejected";
  request.result = result;
  return NextResponse.json(request);
}
