// Traduções da interface. Para adicionar um idioma: criar `src/i18n/<código>.ts` tipado como
// `Messages` (faltar uma chave vira erro de compilação) e registrar em `LANGUAGES`.
//
// - `t("chave", { var })` troca `{var}` pelo valor.
// - Plural: chaves `<base>_one` / `<base>_other` (regras do `Intl.PluralRules`), usadas por
//   `t("<base>", { count })` quando existirem.
// - Erros do Rust chegam como JSON `{"code": "...", ...}`: `errorText()` traduz `error.<code>`.
import { createContext, type ComponentChildren } from "preact";
import { useContext, useEffect, useState } from "preact/hooks";
import { en, type MessageKey, type Messages } from "./en";
import { ptBR } from "./pt-BR";

export type { MessageKey } from "./en";

export const LANGUAGES: { code: string; name: string; messages: Messages }[] = [
    { code: "en", name: "English", messages: en },
    { code: "pt-BR", name: "Português (Brasil)", messages: ptBR },
];

/** Escolhe o idioma suportado mais próximo da lista de preferências; inglês se nenhum servir. */
export function resolveLanguage(preferred: readonly string[]): string {
    const codes = LANGUAGES.map((l) => l.code);
    for (const wanted of preferred) {
        const exact = codes.find((code) => code.toLowerCase() === wanted.toLowerCase());
        if (exact) return exact;
        const base = wanted.split("-")[0].toLowerCase();
        const sameBase = codes.find((code) => code.split("-")[0].toLowerCase() === base);
        if (sameBase) return sameBase;
    }
    return "en";
}

let current: Messages = en;
let currentCode = "en";
const listeners = new Set<(code: string) => void>();

export function setLanguage(code: string) {
    const language = LANGUAGES.find((l) => l.code === code) ?? LANGUAGES[0];
    document.documentElement.lang = language.code;
    if (language.code === currentCode) return;
    current = language.messages;
    currentCode = language.code;
    listeners.forEach((listener) => listener(language.code));
}

export const currentLanguage = () => currentCode;

let systemLanguages: readonly string[] = [];

/** Idioma do Windows (ou do navegador, na página de quem assiste), já resolvido para um suportado. */
export const systemLanguage = () => resolveLanguage(systemLanguages);

/** Guarda os idiomas do sistema e devolve o idioma inicial (`configured` = o das configurações). */
export function initLanguages(system: readonly string[], configured: string | null) {
    systemLanguages = system;
    return configured && LANGUAGES.some((l) => l.code === configured) ? configured : systemLanguage();
}

/** `null` = seguir o sistema. */
export function applyLanguageSetting(configured: string | null) {
    setLanguage(configured && LANGUAGES.some((l) => l.code === configured) ? configured : systemLanguage());
}

type Vars = Record<string, string | number>;

function lookup(key: string): string | undefined {
    return (current as Record<string, string>)[key] ?? (en as Record<string, string>)[key];
}

/** Texto traduzido. Usar dentro de componentes via `useT()` para re-renderizar ao trocar o idioma. */
export function t(key: MessageKey | PluralKey, vars?: Vars): string {
    let text: string | undefined;
    if (vars && typeof vars.count === "number") {
        const rule = new Intl.PluralRules(currentCode).select(vars.count);
        text = lookup(`${key}_${rule}`) ?? lookup(`${key}_other`);
    }
    text ??= lookup(key) ?? key;
    if (!vars) return text;
    return text.replace(/\{(\w+)\}/g, (match, name: string) => (name in vars ? formatVar(vars[name]) : match));
}

/** Como `t`, mas as variáveis podem ser elementos (`{ value: <b>3 Mbps</b> }`). */
export function tx(key: MessageKey, vars: Record<string, ComponentChildren>): ComponentChildren[] {
    const text = t(key);
    return text.split(/\{(\w+)\}/).map((part, i) => (i % 2 === 1 ? (part in vars ? vars[part] : `{${part}}`) : part));
}

// Sem separador de milhar: portas e contagens ficam legíveis ("8787", não "8.787").
const formatVar = (value: string | number) => (typeof value === "number" ? new Intl.NumberFormat(currentCode, { useGrouping: false }).format(value) : value);

/** Bases das chaves com plural (`viewers_one` → `viewers`). */
type PluralKey = MessageKey extends infer K ? (K extends `${infer Base}_${"one" | "other"}` ? Base : never) : never;

/** Mensagem de erro (string, Error ou erro com código vindo do Rust) no idioma atual. */
export function errorText(error: unknown): string {
    const text = typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
    if (!text.startsWith("{")) return text;
    try {
        const { code, detail, ...data } = JSON.parse(text) as { code?: string; detail?: string } & Vars;
        if (!code) return text;
        const message = t(`error.${code}` as MessageKey, data);
        return detail ? `${message} (${detail})` : message;
    } catch {
        return text;
    }
}

const LanguageContext = createContext("en");

/** Componentes que mostram texto chamam isto para atualizar quando o idioma muda. */
export function useT() {
    useContext(LanguageContext);
    return t;
}

export function LanguageProvider({ initial, children }: { initial: string; children: ComponentChildren }) {
    const [code, setCode] = useState(() => {
        setLanguage(initial);
        return currentCode;
    });
    useEffect(() => {
        listeners.add(setCode);
        return () => void listeners.delete(setCode);
    }, []);
    return <LanguageContext.Provider value={code}>{children}</LanguageContext.Provider>;
}
