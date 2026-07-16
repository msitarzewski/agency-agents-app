import en, { type MessageKey, type Messages } from "./locales/en";
import zhCN from "./locales/zh-CN";
import zhTW from "./locales/zh-TW";
import ja from "./locales/ja";
import ko from "./locales/ko";
import es from "./locales/es";
import fr from "./locales/fr";
import de from "./locales/de";
import ptBR from "./locales/pt-BR";
import ru from "./locales/ru";
import fa from "./locales/fa;"

export const LOCALES = ["en", "zh-CN", "zh-TW", "ja", "ko", "es", "fr", "de", "pt-BR", "ru", "fa"] as const;
export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "en";

export function isLocale(value: string): value is Locale {
  return (LOCALES as readonly string[]).includes(value);
}

export const localeLabels: Record<Locale, string> = {
  en: "English",
  "zh-CN": "简体中文",
  "zh-TW": "繁體中文",
  ja: "日本語",
  ko: "한국어",
  es: "Español",
  fr: "Français",
  de: "Deutsch",
  "pt-BR": "Português (Brasil)",
  ru: "Русский",
  fa: "فارسی"
};

const overrides = {
  en: {},
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  ja,
  ko,
  es,
  fr,
  de,
  "pt-BR": ptBR,
  ru,
  fa,
} satisfies Record<Locale, Partial<Messages>>;

export const messages: Record<Locale, Messages> = Object.fromEntries(
  LOCALES.map((locale) => [locale, { ...en, ...overrides[locale] }]),
) as Record<Locale, Messages>;

export type { MessageKey, Messages };
