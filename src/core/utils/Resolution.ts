import { RESOLUTION_LIST } from "./constants";

type resolutionType = { width: number; height: number };

export default class Resolution {
    static getFromTag(tag: string | null): resolutionType | null {
        if (!tag) return null;
        const resolution = RESOLUTION_LIST.find((res) => res.tag === tag);
        if (!resolution) return null;
        return { width: resolution.width, height: resolution.height };
    }

    static getApproximateTag(resolution: resolutionType): { text: string; color: string } | null {
        const maxHeight = Math.max(...RESOLUTION_LIST.map((r) => r.height));
        const minHeight = Math.min(...RESOLUTION_LIST.map((r) => r.height));

        // Se o height não existir na lista, é uma resolução Custom
        if (resolution.height > maxHeight || resolution.height < minHeight) {
            return { text: "Custom", color: "goldenrod" };
        }

        // Filtra resoluções com height >= resolution.height
        let validByHeight = RESOLUTION_LIST.filter((res) => res.height >= resolution.height);

        // Entre as válidas por height, encontra a menor height
        const minValidHeight = Math.min(...validByHeight.map((r) => r.height));
        const sameHeight = validByHeight.filter((r) => r.height === minValidHeight);

        // ORDENA por width para garantir a ordem correta
        sameHeight.sort((a, b) => a.width - b.width);

        // Se tem várias com a mesma height, pega a próxima maior ou igual em width
        let closestResolution = sameHeight[sameHeight.length - 1]; // maior como fallback

        for (const res of sameHeight) {
            if (res.width >= resolution.width) {
                closestResolution = res;
                break;
            }
        }

        return { text: closestResolution.tag, color: closestResolution.color };
    }

    static getResolutions() {
        return RESOLUTION_LIST;
    }

    static getAspectRatio(resolution: resolutionType) {
        const gcd = (a: number, b: number) => {
            while (b) {
                a %= b;
                [a, b] = [b, a]; // Swap a and b
            }
            return a;
        };

        const commonDivisor = gcd(resolution.width, resolution.height);
        const w = resolution.width / commonDivisor;
        const h = resolution.height / commonDivisor;
        return `${w}:${h}`;
    }
}
