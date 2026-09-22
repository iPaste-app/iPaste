import type { MfaAccount, MfaAccountInput, MfaAlgorithm } from "../types";

export type ParsedOtpAuth = Required<Pick<MfaAccountInput, "name" | "secret" | "algorithm" | "digits" | "period">>
  & Pick<MfaAccountInput, "issuer" | "description" | "sourceUri">;

const ALGORITHM_BY_MFA: Record<MfaAlgorithm, HmacImportParams> = {
  SHA1: { name: "HMAC", hash: "SHA-1" },
  SHA256: { name: "HMAC", hash: "SHA-256" },
  SHA512: { name: "HMAC", hash: "SHA-512" },
};

export function parseOtpAuthUri(value: string): ParsedOtpAuth | null {
  const trimmed = value.trim();
  if (!trimmed.toLowerCase().startsWith("otpauth://")) return null;

  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return null;
  }

  if (url.hostname.toLowerCase() !== "totp") return null;

  const params = url.searchParams;
  const secret = normalizeMfaSecret(params.get("secret") ?? "");
  if (!isValidMfaSecret(secret)) return null;

  const rawLabel = decodeOtpLabel(url.pathname.replace(/^\/+/, ""));
  const issuerFromLabel = rawLabel.includes(":") ? rawLabel.split(":")[0]?.trim() : "";
  const nameFromLabel = rawLabel.includes(":") ? rawLabel.split(":").slice(1).join(":").trim() : rawLabel.trim();
  const issuer = params.get("issuer")?.trim() || issuerFromLabel || null;
  const name = nameFromLabel || issuer || "MFA";
  const algorithm = normalizeMfaAlgorithm(params.get("algorithm"));
  const digits = normalizeMfaDigits(params.get("digits"));
  const period = normalizeMfaPeriod(params.get("period"));

  return {
    name,
    issuer,
    description: null,
    secret,
    algorithm,
    digits,
    period,
    sourceUri: trimmed,
  };
}

export function normalizeMfaSecret(secret: string) {
  return secret.replace(/[\s=-]/g, "").toUpperCase();
}

export function isValidMfaSecret(secret: string) {
  return secret.length >= 8 && /^[A-Z2-7]+$/.test(secret);
}

export function normalizeMfaAlgorithm(value: string | null | undefined): MfaAlgorithm {
  const normalized = (value || "SHA1").replace("-", "").toUpperCase();
  if (normalized === "SHA256" || normalized === "SHA512") return normalized;
  return "SHA1";
}

export function normalizeMfaDigits(value: string | number | null | undefined) {
  const digits = Number(value);
  return Number.isInteger(digits) && digits >= 6 && digits <= 8 ? digits : 6;
}

export function normalizeMfaPeriod(value: string | number | null | undefined) {
  const period = Number(value);
  return Number.isInteger(period) && period >= 10 && period <= 120 ? period : 30;
}

export function mfaInputFromAccount(account: MfaAccount): MfaAccountInput {
  return {
    name: account.name,
    issuer: account.issuer ?? null,
    description: account.description ?? null,
    secret: account.secret,
    algorithm: account.algorithm,
    digits: account.digits,
    period: account.period,
    sourceUri: account.sourceUri ?? null,
  };
}

export async function generateTotp(account: Pick<MfaAccount, "secret" | "algorithm" | "digits" | "period">, timestamp = Date.now()) {
  const secretBytes = base32ToBytes(account.secret);
  const counter = Math.floor(timestamp / 1000 / account.period);
  const counterBytes = counterToBytes(counter);
  const key = await crypto.subtle.importKey("raw", secretBytes, ALGORITHM_BY_MFA[account.algorithm], false, ["sign"]);
  const signature = new Uint8Array(await crypto.subtle.sign("HMAC", key, counterBytes));
  const offset = signature[signature.length - 1] & 0x0f;
  const binary = ((signature[offset] & 0x7f) << 24)
    | ((signature[offset + 1] & 0xff) << 16)
    | ((signature[offset + 2] & 0xff) << 8)
    | (signature[offset + 3] & 0xff);
  const modulo = 10 ** account.digits;
  return String(binary % modulo).padStart(account.digits, "0");
}

export function secondsRemaining(period: number, timestamp = Date.now()) {
  const seconds = Math.floor(timestamp / 1000);
  const remaining = period - (seconds % period);
  return remaining === 0 ? period : remaining;
}

function decodeOtpLabel(value: string) {
  try {
    return decodeURIComponent(value.replace(/\+/g, "%20"));
  } catch {
    return value;
  }
}

function base32ToBytes(value: string) {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  const clean = normalizeMfaSecret(value);
  let bits = "";
  for (const char of clean) {
    const index = alphabet.indexOf(char);
    if (index < 0) throw new Error("Invalid Base32 secret");
    bits += index.toString(2).padStart(5, "0");
  }

  const bytes: number[] = [];
  for (let index = 0; index + 8 <= bits.length; index += 8) {
    bytes.push(Number.parseInt(bits.slice(index, index + 8), 2));
  }
  return new Uint8Array(bytes);
}

function counterToBytes(counter: number) {
  const bytes = new ArrayBuffer(8);
  const view = new DataView(bytes);
  const high = Math.floor(counter / 0x100000000);
  const low = counter >>> 0;
  view.setUint32(0, high);
  view.setUint32(4, low);
  return bytes;
}
