import { RESOLUTION_LIST } from "./constants";

type resolutionType = { width: number; height: number };
export type ResolutionTag = { text: string; color: string };

/** Cor da bandeirinha de uma resolução sem nome padrão (só a proporção). */
const NON_STANDARD_COLOR = "#606060";

// Proporções comuns; resoluções "quase" nelas (1366x768, 1600x896 da webcam…) usam o nome comum.
const COMMON_ASPECTS: [string, number][] = [
    ["32:9", 32 / 9],
    ["21:9", 21 / 9],
    ["2:1", 2],
    ["17:9", 17 / 9],
    ["16:9", 16 / 9],
    ["5:3", 5 / 3],
    ["16:10", 16 / 10],
    ["3:2", 3 / 2],
    ["4:3", 4 / 3],
    ["5:4", 5 / 4],
    ["11:9", 11 / 9],
    ["1:1", 1],
];
const ASPECT_TOLERANCE = 0.025;

export default class Resolution {
    static getFromTag(tag: string | null): resolutionType | null {
        if (!tag) return null;
        const resolution = RESOLUTION_LIST.find((res) => res.tag === tag);
        if (!resolution) return null;
        return { width: resolution.width, height: resolution.height };
    }

    /** Bandeirinha de nome padrão (Full HD, WQHD…), só se a resolução for exatamente uma delas. */
    static getStandardTag(resolution: resolutionType): ResolutionTag | null {
        const exact = RESOLUTION_LIST.find((r) => r.width === resolution.width && r.height === resolution.height);
        return exact ? { text: exact.tag, color: exact.color } : null;
    }

    /** Nome padrão; sem nome, a proporção em cinza (ex.: 1600x896 → "16:9"). */
    static getTag(resolution: resolutionType): ResolutionTag {
        return Resolution.getStandardTag(resolution) ?? { text: Resolution.getAspectRatio(resolution), color: NON_STANDARD_COLOR };
    }

    static getResolutions() {
        return RESOLUTION_LIST;
    }

    /** Proporção, arredondada para a comum mais próxima quando bem perto (1366x768 → 16:9). */
    static getAspectRatio(resolution: resolutionType) {
        const ratio = resolution.width / resolution.height;
        const common = COMMON_ASPECTS.find(([, value]) => Math.abs(ratio - value) / value < ASPECT_TOLERANCE);
        if (common) return common[0];

        const gcd = (a: number, b: number) => {
            while (b) {
                a %= b;
                [a, b] = [b, a];
            }
            return a;
        };
        const divisor = gcd(resolution.width, resolution.height);
        return `${resolution.width / divisor}:${resolution.height / divisor}`;
    }
}
