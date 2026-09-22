import assert from "node:assert/strict";
import { test } from "node:test";
import { generateTotp, parseOtpAuthUri, secondsRemaining } from "../src/lib/mfa";
import { filterMfaAccounts } from "../src/lib/mfaList";
import type { MfaAccount } from "../src/types";
import { shortcutsEqual } from "../src/lib/shortcuts";
import { en } from "../src/i18n/locales/en";
import { zhCN } from "../src/i18n/locales/zh-CN";
import { ja } from "../src/i18n/locales/ja";
import { ko } from "../src/i18n/locales/ko";
import { es } from "../src/i18n/locales/es";
import { fr } from "../src/i18n/locales/fr";
import { de } from "../src/i18n/locales/de";

function base32(value: string) {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  const bits = [...Buffer.from(value)].map(byte => byte.toString(2).padStart(8, "0")).join("");
  return bits.match(/.{1,5}/g)!.map(chunk => alphabet[parseInt(chunk.padEnd(5, "0"), 2)]).join("");
}

test("MFA search combines issuer, account and notes without searching secrets", () => {
  const accounts = [
    { id: "a", issuer: "GitHub", name: "alice@example.com", description: "Work", secret: "PRIVATESECRET", sourceUri: "otpauth://private" },
    { id: "b", issuer: "GitHub", name: "bob@example.com", description: "Personal" },
  ] as MfaAccount[];
  assert.deepEqual(filterMfaAccounts(accounts, "  GITHUB alice WORK ").map(item => item.id), ["a"]);
  assert.deepEqual(filterMfaAccounts(accounts, "personal").map(item => item.id), ["b"]);
  assert.deepEqual(filterMfaAccounts(accounts, "missing"), []);
  assert.deepEqual(filterMfaAccounts(accounts, "PRIVATESECRET"), []);
  assert.deepEqual(filterMfaAccounts(accounts, "otpauth"), []);
  assert.equal(filterMfaAccounts(accounts, "  "), accounts);
});

test("shortcut comparison resolves native aliases without conflating Control and Command", () => {
  assert.ok(shortcutsEqual("CommandOrControl+Shift+A", "Shift+Command+A", true));
  assert.ok(shortcutsEqual("CommandOrControl+Shift+A", "Control+Shift+A", false));
  assert.ok(!shortcutsEqual("Control+Shift+A", "Command+Shift+A", true));
  assert.ok(!shortcutsEqual("CommandOrControl+Shift+A", "CommandOrControl+Shift+V", true));
});

test("TOTP matches all RFC 6238 SHA1, SHA256 and SHA512 test vectors", async () => {
  const secrets = {
    SHA1: "12345678901234567890",
    SHA256: "12345678901234567890123456789012",
    SHA512: "1234567890123456789012345678901234567890123456789012345678901234",
  };
  const vectors = [
    [59, "94287082", "46119246", "90693936"],
    [1111111109, "07081804", "68084774", "25091201"],
    [1111111111, "14050471", "67062674", "99943326"],
    [1234567890, "89005924", "91819424", "93441116"],
    [2000000000, "69279037", "90698825", "38618901"],
    [20000000000, "65353130", "77737706", "47863826"],
  ] as const;
  for (const [seconds, ...expected] of vectors) {
    for (const [index, algorithm] of (["SHA1", "SHA256", "SHA512"] as const).entries()) {
      assert.equal(await generateTotp({ secret: base32(secrets[algorithm]), algorithm, digits: 8, period: 30 }, seconds * 1000), expected[index]);
    }
  }
});

test("otpauth imports preserve issuer, identity and custom generation parameters", () => {
  const parsed = parseOtpAuthUri("otpauth://totp/Example%3Aalice%40example.com?secret=jbswy3dpehpk3pxp&issuer=Example&algorithm=SHA256&digits=8&period=60");
  assert.equal(parsed?.name, "alice@example.com");
  assert.equal(parsed?.issuer, "Example");
  assert.equal(parsed?.secret, "JBSWY3DPEHPK3PXP");
  assert.equal(parsed?.algorithm, "SHA256");
  assert.equal(parsed?.digits, 8);
  assert.equal(parsed?.period, 60);
  for (const invalid of ["https://example.com", "otpauth://hotp/test?secret=JBSWY3DPEHPK3PXP", "otpauth://totp/test?secret=invalid!"]) {
    assert.equal(parseOtpAuthUri(invalid), null);
  }
});

test("countdown and code change at the same period boundary", async () => {
  const account = { secret: base32("12345678901234567890"), algorithm: "SHA1", digits: 6, period: 30 } as const;
  assert.equal(secondsRemaining(30, 59999), 1);
  assert.equal(secondsRemaining(30, 60000), 30);
  assert.notEqual(await generateTotp(account, 59999), await generateTotp(account, 60000));
});

test("split locales retain both latest main and application center translations", () => {
  for (const locale of [zhCN, ja, ko, es, fr, de]) {
    assert.deepEqual(Object.keys(locale).sort(), Object.keys(en).sort());
  }
  for (const key of ["appCenter.title", "apps.mfa.name", "settings.shortcuts.appCenter.title", "settings.loadError", "viewer.ocrLoading.paddle"]) {
    assert.ok(en[key], key);
    assert.ok(zhCN[key], key);
  }
});
