// Seletores de resolução e FPS com as bandeirinhas do app (mesmas listas da barra superior),
// mais a opção "Native" (igual à fonte).
import { DropDown, DropOption } from "../../core/components/DropDown";
import Tag from "../../core/components/Tag";
import Resolution from "../../core/utils/Resolution";
import { FRAMERATE_LIST, RESOLUTION_LIST } from "../../core/utils/constants";
import type { SourceInfo } from "../../core/media/presets";
import { fpsColor, formatFps } from "../../core/utils/videoModes";

const NATIVE = "native";

export type Size = { width: number; height: number };

const key = (size: Size) => `${size.width}x${size.height}`;

/** `value = null` = nativo. Só oferece resoluções até a da fonte (nunca aumenta a imagem). */
export function ResolutionPicker({ value, source, onChange }: { value: Size | null; source: SourceInfo; onChange: (value: Size | null) => void }) {
    const nativeTag = Resolution.getTag(source);
    const options = RESOLUTION_LIST.filter((r) => r.width <= source.width && r.height <= source.height);
    const selected = value ? key(value) : NATIVE;
    // Valor salvo fora da lista (ex.: a fonte mudou): continua aparecendo.
    const custom = value && !options.some((r) => key(r) === selected) ? value : null;
    return (
        <DropDown
            title="Resolution"
            value={selected}
            onChoose={(v) => {
                if (v === NATIVE) return onChange(null);
                const [width, height] = v.split("x").map(Number);
                onChange({ width, height });
            }}>
            <DropOption value={NATIVE} className="flex gap-3">
                {`Native (${source.width} x ${source.height})`}
                {nativeTag && <Tag text={nativeTag.text} color={nativeTag.color} />}
            </DropOption>
            {custom && (
                <DropOption value={key(custom)} className="flex gap-3">
                    {`${custom.width} x ${custom.height}`}
                </DropOption>
            )}
            {options.map((r) => (
                <DropOption key={key(r)} value={key(r)} className="flex gap-3">
                    {`${r.width} x ${r.height}`}
                    <Tag text={r.tag} color={r.color} />
                </DropOption>
            ))}
        </DropDown>
    );
}

/** `value = null` = nativo. */
export function FpsPicker({ value, source, onChange }: { value: number | null; source: SourceInfo; onChange: (value: number | null) => void }) {
    const sourceFps = Math.round(source.fps);
    const options = FRAMERATE_LIST.filter((f) => f <= sourceFps || f === value);
    return (
        <DropDown title="Framerate" value={value ? String(value) : NATIVE} onChoose={(v) => onChange(v === NATIVE ? null : Number(v))}>
            <DropOption value={NATIVE} className="flex gap-3">
                Native
                <Tag text={`${formatFps(sourceFps)} FPS`} color={fpsColor(sourceFps)} />
            </DropOption>
            {[...options].map((f) => (
                <DropOption key={f} value={String(f)} className="flex gap-3">
                    <Tag text={`${f} FPS`} color={fpsColor(f)} />
                </DropOption>
            ))}
        </DropDown>
    );
}
