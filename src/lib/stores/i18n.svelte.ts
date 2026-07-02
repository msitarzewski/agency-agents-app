import { DEFAULT_LOCALE, LOCALES, isLocale, messages, type Locale, type MessageKey } from "$lib/i18n/messages";

const STORAGE_KEY = "agency-agents:locale";

function matchLocale(value: string | null | undefined): Locale | null {
  if (!value) return null;
  const lang = value.replace("_", "-").toLowerCase();
  const exact = LOCALES.find((locale) => locale.toLowerCase() === lang);
  if (exact) return exact;
  if (lang.startsWith("zh")) return lang.includes("tw") || lang.includes("hk") || lang.includes("hant") ? "zh-TW" : "zh-CN";
  if (lang.startsWith("pt")) return "pt-BR";
  return LOCALES.find((locale) => locale !== "en" && lang.startsWith(locale.toLowerCase())) ?? null;
}

function detectLocale(): Locale {
  if (typeof localStorage !== "undefined") {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && isLocale(saved)) return saved;
  }
  if (typeof navigator !== "undefined") {
    const langs = navigator.languages?.length ? navigator.languages : [navigator.language];
    for (const lang of langs) {
      const match = matchLocale(lang);
      if (match) return match;
    }
  }
  return DEFAULT_LOCALE;
}

function applyLocale(locale: Locale) {
  if (typeof document !== "undefined") document.documentElement.lang = locale;
}

function format(template: string, vars?: Record<string, string | number>): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (match, key) => String(vars[key] ?? match));
}

class I18nStore {
  locale: Locale = $state(DEFAULT_LOCALE);

  init() {
    this.locale = detectLocale();
    applyLocale(this.locale);
  }

  setLocale(locale: Locale) {
    this.locale = locale;
    applyLocale(locale);
    if (typeof localStorage !== "undefined") localStorage.setItem(STORAGE_KEY, locale);
  }

  t(key: MessageKey, vars?: Record<string, string | number>): string {
    return format(messages[this.locale][key] ?? messages[DEFAULT_LOCALE][key], vars);
  }

  optional(key: string, fallback: string, vars?: Record<string, string | number>): string {
    const local = messages[this.locale] as Record<string, string>;
    const en = messages[DEFAULT_LOCALE] as Record<string, string>;
    return format(local[key] ?? en[key] ?? fallback, vars);
  }

  count(count: number, one: MessageKey, many: MessageKey, vars?: Record<string, string | number>): string {
    return this.t(count === 1 ? one : many, { count, ...(vars ?? {}) });
  }
}

export const i18n = new I18nStore();
