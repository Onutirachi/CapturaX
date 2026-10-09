import { ComponentChildren } from "preact";
import { useState, useEffect, useLayoutEffect, useRef } from "preact/hooks";
import { Children } from "preact/compat";
import ScrollText from "./ScrollText";

type DropDownProps = {
    children: ComponentChildren;
    title: string;
    onChoose?: (value: string) => void; // eslint-disable-line
    /** Chave do localStorage (padrão: o título). */
    storageKey?: string;
    /** Modo controlado: o valor vem de fora (ex.: config do Rust) e nada é salvo aqui. */
    value?: string | null;
    /** Ocupa a largura toda do pai (ex.: janelas); sem isso, cresce até um limite e o texto rola. */
    fullWidth?: boolean;
};

/** Evento global para fechar dropdowns abertos (ex.: quando a barra que os contém se esconde). */
export const CLOSE_DROPDOWNS_EVENT = "capturax:close-dropdowns";

type DropOptionProps = {
    children: ComponentChildren;
    className?: string;
    value: string;
};

const isText = (child: unknown) => typeof child === "string" || typeof child === "number";

/**
 * Uma célula de texto (rola se não couber) seguida das bandeirinhas. O próprio elemento não gera
 * caixa (`contents`): no menu aberto, texto e bandeirinhas viram colunas alinhadas entre as opções.
 */
export function DropOption({ children, className, value }: DropOptionProps) {
    const parts = Children.toArray(children);
    const text = parts.filter(isText).join("");
    return (
        <div data-value={value} className={`contents ${className ? className : ""}`}>
            {text ? <ScrollText>{text}</ScrollText> : <span />}
            {parts.filter((child) => !isText(child))}
        </div>
    );
}

/** Quantas bandeirinhas (colunas além do texto) a opção com mais delas tem. */
function tagColumns(children: ComponentChildren) {
    return Math.max(0, ...(Children.toArray(children) as any[]).map((child) => Children.toArray(child?.props?.children).filter((part) => !isText(part)).length));
}

export function DropDown({ children, title, onChoose = (_) => {}, storageKey = title, value: controlledValue, fullWidth = false }: DropDownProps) {
    const controlled = controlledValue !== undefined;
    const dropDownRef = useRef<HTMLDivElement | null>(null);
    const [open, setOpen] = useState<boolean>(false);
    const [value, setValue] = useState<string | null>(null);
    const [display, setDisplay] = useState<any | null>(null);
    /** Menu alinhado pela direita quando não cabe à direita do dropdown. */
    const [alignRight, setAlignRight] = useState(false);
    const menuRef = useRef<HTMLDivElement | null>(null);

    const toggleOpen = () => {
        setOpen(!open);
    };

    const selectOption = (child: any) => {
        setOpen(false);
        if (controlled) {
            if (child.props.value !== controlledValue) onChoose(child.props.value);
            return;
        }
        setDisplay(child);
        setValue(child.props.value);
        localStorage.setItem(storageKey, child.props.value);
    };

    // Só re-seleciona quando as opções mudam de verdade (ex.: dispositivo plugado/desplugado).
    // `children` é um array novo a cada render do pai, então não pode ser a dependência.
    const optionValues = (Children.toArray(children) as any[]).map((child) => child.props?.value).join("\u0000");

    useEffect(() => {
        if (controlled) return;
        const storedValue = localStorage.getItem(storageKey);
        const optionsArray = Children.toArray(children) as any;

        if (storedValue) {
            const found = optionsArray.find((child: any) => child.props?.value === storedValue);
            if (found) {
                selectOption(found);
                return;
            }
        }

        if (optionsArray.length === 0) return;
        selectOption(optionsArray[0]);
    }, [optionValues]);

    useEffect(() => {
        if (controlled || !value) return;
        onChoose(value);
    }, [value]);

    const shown = controlled ? (Children.toArray(children) as any[]).find((child) => child.props?.value === controlledValue) ?? null : display;

    useLayoutEffect(() => {
        if (!open || !menuRef.current || !dropDownRef.current) return;
        const box = dropDownRef.current.getBoundingClientRect();
        setAlignRight(box.left + menuRef.current.offsetWidth > window.innerWidth - 8);
    }, [open]);

    useEffect(() => {
        if (!open) return;
        // Captura no document roda antes do listener de Esc do fullscreen (window, bubble).
        const handler = (e: KeyboardEvent) => {
            if (e.key !== "Escape") return;
            e.stopPropagation();
            setOpen(false);
        };
        document.addEventListener("keydown", handler, true);
        return () => document.removeEventListener("keydown", handler, true);
    }, [open]);

    useEffect(() => {
        const close = () => setOpen(false);
        window.addEventListener(CLOSE_DROPDOWNS_EVENT, close);
        return () => window.removeEventListener(CLOSE_DROPDOWNS_EVENT, close);
    }, []);

    useEffect(() => {
        const handler = (e: MouseEvent) => {
            if (dropDownRef.current && !dropDownRef.current.contains(e.target as any)) {
                setOpen(false);
            }
        };

        document.addEventListener("click", handler);
        return () => {
            document.removeEventListener("click", handler);
        };
    }, [dropDownRef]);

    return (
        <div data-value={controlled ? controlledValue : value} className={`h-11 relative ${fullWidth ? "w-full" : "w-auto min-w-48 max-w-64"}`} ref={dropDownRef}>
            <div className="absolute bg-normal -translate-y-[50%] translate-x-[20%] px-1 rounded-sm text-sm z-1">{title}</div>
            <div
                className="w-full h-full bg-dark [border:var(--border)] rounded-xl cursor-pointer overflow-hidden flex items-center gap-3 px-3 relative"
                onClick={toggleOpen}>
                {/* Todas as opções empilhadas na mesma célula (só a escolhida visível): a largura do
                    dropdown é sempre a da maior opção, sem quebrar linha nem pular ao trocar. */}
                <div className="grid grid-cols-[minmax(0,1fr)] min-w-0 flex-1">
                    {Children.map(children, (child: any) => (
                        <div className={`[grid-area:1/1] min-w-0 whitespace-nowrap flex items-center gap-3 [&>:first-child]:flex-1 [&>*]:min-w-0 ${child === shown || (shown && child?.props?.value === (shown as any).props?.value) ? "" : "invisible"}`}>
                            {child}
                        </div>
                    ))}
                </div>
                <div
                    className={`h-[50%] aspect-square ml-auto shrink-0 bg-center mask-contain mask-no-repeat mask-[url(./assets/DropDown/chevron.svg)] bg-white transition ${
                        open ? "-rotate-180" : "rotate-0"
                    }`}></div>
            </div>
            <div
                ref={menuRef}
                className={`
                    z-2
                    absolute top-full ${alignRight ? "right-0" : "left-0"}
                    w-max min-w-full max-w-[min(28rem,calc(100vw-2rem))] h-auto
                    p-1.5 mt-2
                    rounded-xl bg-dark 
                    box-border
                    transition
                    ${open ? "visibility-visible opacity-100 translate-y-0" : "visibility-hidden opacity-0 pointer-events-none -translate-y-2.5"}
                `}>
                <div
                    className="w-full h-max max-h-100 overflow-y-auto overflow-x-hidden grid auto-rows-auto"
                    style={{ gridTemplateColumns: `minmax(0, 1fr) repeat(${tagColumns(children)}, max-content)` }}>
                    {Children.map(children, (child: any) => {
                        return (
                            <div
                                key={child.props.value}
                                onClick={() => {
                                    selectOption(child);
                                }}
                                className="scroll-on-hover col-span-full grid grid-cols-subgrid gap-x-3 items-center bg-transparent hover:bg-normal cursor-pointer p-1.5 rounded-md box-border whitespace-nowrap min-w-0 [&_.tag]:w-auto">
                                {child}
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}
