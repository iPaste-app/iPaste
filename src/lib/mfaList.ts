import type { MfaAccount } from "../types";

// Search only display metadata, never secrets or the otpauth source URI.
export function filterMfaAccounts(accounts: MfaAccount[], query: string): MfaAccount[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return accounts;
  return accounts.filter((account) => {
    const label = [account.issuer, account.name, account.description].filter(Boolean).join(" ").toLocaleLowerCase();
    return terms.every((term) => label.includes(term));
  });
}
