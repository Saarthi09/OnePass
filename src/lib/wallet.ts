import type { WalletEntry } from "./types";

// The wallet lives in the user's own browser. OnePass servers never store it.
const KEY = "onepass.wallet";

export function loadWallet(): WalletEntry[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]");
  } catch {
    return [];
  }
}

export function saveWallet(entries: WalletEntry[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(entries));
  } catch {
    /* storage unavailable: wallet stays in memory for this page only */
  }
}

export const CLAIM_LABELS: Record<string, string> = {
  name: "Full name",
  studentId: "Student ID",
  birthDate: "Date of birth",
  university: "University",
  isStudent: "Is a current student",
  over18: "Is over 18",
};

export function show(value: unknown): string {
  return typeof value === "boolean" ? (value ? "Yes" : "No") : String(value);
}
