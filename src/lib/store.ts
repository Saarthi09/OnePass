import type { VerificationRequest } from "./types";

// In-memory store for pending requests. Fine for a demo; resets when the server restarts.
const g = globalThis as unknown as { __requests?: Map<string, VerificationRequest> };
export const requests: Map<string, VerificationRequest> = (g.__requests ??= new Map());
