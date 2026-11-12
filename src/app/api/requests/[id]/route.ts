import { NextResponse } from "next/server";
import { requests } from "@/lib/store";

export async function GET(_req: Request, { params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const request = requests.get(id);
  if (!request) return NextResponse.json({ error: "Request not found" }, { status: 404 });
  return NextResponse.json(request);
}
