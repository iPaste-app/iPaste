import type { I18nKey } from "../i18n";

export type BuiltinAppId = "mfa-manager";
export type AppKind = "builtin" | "user" | "marketplace";

export type AppDefinition = {
  id: BuiltinAppId;
  kind: AppKind;
  nameKey: I18nKey;
  descriptionKey: I18nKey;
  statusKey: I18nKey;
  accent: string;
  capabilities: I18nKey[];
};

export const builtinApps: AppDefinition[] = [
  {
    id: "mfa-manager",
    kind: "builtin",
    nameKey: "apps.mfa.name",
    descriptionKey: "apps.mfa.description",
    statusKey: "appCenter.builtin",
    accent: "#0D9488",
    capabilities: [
      "apps.mfa.capability.clipboardQr",
      "apps.mfa.capability.historyScan",
      "apps.mfa.capability.quickPaste",
    ],
  },
];
