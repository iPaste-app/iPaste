import { de } from "./locales/de";
import { en, type I18nKey } from "./locales/en";
import { es } from "./locales/es";
import { fr } from "./locales/fr";
import { id } from "./locales/id";
import { it } from "./locales/it";
import { ja } from "./locales/ja";
import { ko } from "./locales/ko";
import { nl } from "./locales/nl";
import { pl } from "./locales/pl";
import { ptBR } from "./locales/pt-BR";
import { ru } from "./locales/ru";
import { th } from "./locales/th";
import { tr } from "./locales/tr";
import { vi } from "./locales/vi";
import { zhCN } from "./locales/zh-CN";
import { zhTW } from "./locales/zh-TW";

type Catalog = { label: string; locale?: string; messages: Record<I18nKey, string> };

export const catalogs = {
  en: { label: "English", locale: "en-US", messages: en },
  "zh-CN": { label: "简体中文", messages: zhCN },
  "zh-TW": { label: "繁體中文", messages: zhTW },
  ja: { label: "日本語", locale: "ja-JP", messages: ja },
  ko: { label: "한국어", locale: "ko-KR", messages: ko },
  es: { label: "Español", messages: es },
  fr: { label: "Français", locale: "fr-FR", messages: fr },
  de: { label: "Deutsch", locale: "de-DE", messages: de },
  "pt-BR": { label: "Português (Brasil)", messages: ptBR },
  it: { label: "Italiano", messages: it },
  ru: { label: "Русский", messages: ru },
  pl: { label: "Polski", messages: pl },
  tr: { label: "Türkçe", messages: tr },
  nl: { label: "Nederlands", messages: nl },
  id: { label: "Bahasa Indonesia", messages: id },
  vi: { label: "Tiếng Việt", messages: vi },
  th: { label: "ไทย", messages: th },
} satisfies Record<string, Catalog>;

export type Language = keyof typeof catalogs;

export function getCatalog(language: Language): Catalog {
  return catalogs[language];
}

export function isSupportedLanguage(value: unknown): value is Language {
  return typeof value === "string" && Object.prototype.hasOwnProperty.call(catalogs, value);
}
