import { NextResponse } from "next/server";
import { issueCredential } from "@/lib/credential";
import { getIssuerKeypair, ISSUER_NAME } from "@/lib/issuer";

function isOver18(birthDate: string): boolean {
  const b = new Date(birthDate);
  const cutoff = new Date();
  cutoff.setFullYear(cutoff.getFullYear() - 18);
  return b <= cutoff;
}

// The university's side: turn a student record into a signed credential.
export async function POST(req: Request) {
  const { name, studentId, birthDate } = await req.json();
  if (!name || !studentId || !birthDate || isNaN(new Date(birthDate).getTime())) {
    return NextResponse.json({ error: "name, studentId and birthDate are required" }, { status: 400 });
  }
  const entry = issueCredential(getIssuerKeypair(), ISSUER_NAME, "StudentCredential", {
    name,
    studentId,
    birthDate,
    university: ISSUER_NAME,
    isStudent: true,
    over18: isOver18(birthDate), // lets a user prove their age without revealing their birthday
  });
  return NextResponse.json(entry);
}
