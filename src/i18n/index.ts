import { computed, ref } from "vue";
import { catalogs, getCatalog, isSupportedLanguage, type Language } from "./catalogs";
import type { I18nKey } from "./locales/en";

export type { I18nKey } from "./locales/en";

const STORAGE_KEY = "ipaste.language";
export const DEFAULT_LANGUAGE: Language = "en";

export const languageOptions = (Object.keys(catalogs) as Language[]).map((value) => ({
  value,
  label: catalogs[value].label,
}));

const initialLanguage = cleanLanguage(localStorage.getItem(STORAGE_KEY));
export const currentLanguage = ref<Language>(initialLanguage);

export const currentLocale = computed(() => getCatalog(currentLanguage.value).locale ?? currentLanguage.value);

export function setLanguage(language: Language, options: { persist?: boolean } = {}) {
  currentLanguage.value = cleanLanguage(language);
  if (options.persist ?? true) {
    localStorage.setItem(STORAGE_KEY, currentLanguage.value);
  }
  document.documentElement.lang = currentLanguage.value;
}

export function t(key: I18nKey, params: Record<string, string | number> = {}) {
  const template = getCatalog(currentLanguage.value).messages[key] ?? catalogs.en.messages[key] ?? key;
  return template.replace(/\{(\w+)\}/g, (placeholder, name) =>
    Object.prototype.hasOwnProperty.call(params, name) ? String(params[name]) : placeholder,
  );
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
  return isSupportedLanguage(value) ? value : DEFAULT_LANGUAGE;
}

setLanguage(currentLanguage.value, { persist: false });
