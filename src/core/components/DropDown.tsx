import { ComponentChildren } from "preact";
import { useState, useEffect, useRef } from "preact/hooks";
import { Children } from "preact/compat";

type DropDownProps = {
    children: ComponentChildren;
    title: string;
    onChoose?: (value: string) => void; // eslint-disable-line
};

type DropOptionProps = {
    children: ComponentChildren;
    className?: string;
    value: string;
};

export function DropOption({ children, className, value }: DropOptionProps) {
    return (
        <div data-value={value} className={`${className ? className : ""}`}>
            {children}
        </div>
    );
}

export function DropDown({ children, title, onChoose = (_) => {} }: DropDownProps) {
    const dropDownRef = useRef<HTMLDivElement | null>(null);
    const [open, setOpen] = useState<boolean>(false);
    const [value, setValue] = useState<string | null>(null);
    const [display, setDisplay] = useState<any | null>(null);

    const toggleOpen = () => {
        setOpen(!open);
    };

    const selectOption = (child: any) => {
        setDisplay(child);
        setValue(child.props.value);
        localStorage.setItem(title, child.props.value);
        setOpen(false);
    };

    useEffect(() => {
        const storedValue = localStorage.getItem(title);
        const optionsArray = Children.toArray(children) as any;

        if (storedValue) {
            const found = optionsArray.find((child: any) => child.props?.value === storedValue);
            if (found) {
                selectOption(found);
                return;
            }
        }

        selectOption(optionsArray[0]);
    }, [children]);

    useEffect(() => {
        if (!value) return;
        onChoose(value);
    }, [value]);

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
        <div data-value={value} className="w-auto min-w-3xs h-11 relative" ref={dropDownRef}>
            <div className="absolute bg-normal -translate-y-[50%] translate-x-[20%] px-1 rounded-sm text-sm z-1">{title}</div>
            <div
                className="w-full h-full bg-dark [border:var(--border)] rounded-xl cursor-pointer overflow-hidden flex items-center px-3 relative"
                onClick={toggleOpen}>
                {display}
                <div
                    className={`h-[50%] aspect-square ml-auto bg-center mask-contain mask-no-repeat mask-[url(./assets/DropDown/chevron.svg)] bg-white transition ${
                        open ? "-rotate-180" : "rotate-0"
                    }`}></div>
            </div>
            <div
                className={`
                    absolute top-full left-0
                    w-full h-auto
                    p-3 mt-2 pr-1.5
                    rounded-xl bg-dark 
                    box-border
                    transition
                    ${open ? "visibility-visible opacity-100 translate-y-0" : "visibility-hidden opacity-0 pointer-events-none -translate-y-2.5"}
                `}>
                <div
                    className="
                        w-full h-max max-h-100
                        pr-1.5
                        overflow-y-auto overflow-x-hidden 
                        grid grid-cols-1 auto-rows-auto
                    ">
                    {Children.map(children, (child: any) => {
                        return (
                            <div
                                key={child.props.value}
                                onClick={() => {
                                    selectOption(child);
                                }}
                                className="bg-transparent hover:bg-normal cursor-pointer p-1.5 rounded-md box-border">
                                {child}
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}
