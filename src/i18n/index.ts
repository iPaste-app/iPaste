import { computed, ref } from "vue";
import type { Language } from "../types";
import { de } from "./locales/de";
import { en } from "./locales/en";
import type { I18nKey } from "./locales/en";
import { es } from "./locales/es";
import { fr } from "./locales/fr";
import { ja } from "./locales/ja";
import { ko } from "./locales/ko";
import { zhCN } from "./locales/zh-CN";

export type { I18nKey } from "./locales/en";

const STORAGE_KEY = "ipaste.language";
export const DEFAULT_LANGUAGE: Language = "en";

export const languageOptions: Array<{ value: Language; label: string }> = [
  { value: "en", label: "English" },
  { value: "zh-CN", label: "简体中文" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
  { value: "es", label: "Español" },
  { value: "fr", label: "Français" },
  { value: "de", label: "Deutsch" },
];

const localeByLanguage: Record<Language, string> = {
  en: "en-US",
  "zh-CN": "zh-CN",
  ja: "ja-JP",
  ko: "ko-KR",
  es: "es",
  fr: "fr-FR",
  de: "de-DE",
};

const supportedLanguages = new Set<Language>(languageOptions.map((option) => option.value));

const messages = {
  en,
  "zh-CN": zhCN,
  ja,
  ko,
  es,
  fr,
  de,
} satisfies Record<Language, Record<I18nKey, string>>;

const initialLanguage = cleanLanguage(localStorage.getItem(STORAGE_KEY));
export const currentLanguage = ref<Language>(initialLanguage);

export const currentLocale = computed(() => localeByLanguage[currentLanguage.value] ?? localeByLanguage[DEFAULT_LANGUAGE]);

export function setLanguage(language: Language, options: { persist?: boolean } = {}) {
  currentLanguage.value = cleanLanguage(language);
  if (options.persist ?? true) {
    localStorage.setItem(STORAGE_KEY, currentLanguage.value);
  }
  document.documentElement.lang = currentLanguage.value;
}

export function t(key: I18nKey, params: Record<string, string | number> = {}) {
  const localizedMessages = messagesForLanguage(currentLanguage.value);
  const template = localizedMessages[key] ?? messages.en[key] ?? key;
  return Object.entries(params).reduce(
    (value, [name, replacement]) => value.replace(new RegExp(`\\{${escapeRegExp(name)}\\}`, "g"), String(replacement)),
    template as string,
  );
}

function messagesForLanguage(language: Language): Record<I18nKey, string> {
  return messages[language] ?? messages[DEFAULT_LANGUAGE];
}

export function useI18n() {
  return {
    language: currentLanguage,
    locale: currentLocale,
    t,
    setLanguage,
  };
}

export function cleanLanguage(value: unknown): Language {
  return typeof value === "string" && supportedLanguages.has(value as Language) ? (value as Language) : DEFAULT_LANGUAGE;
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

setLanguage(currentLanguage.value, { persist: false });
