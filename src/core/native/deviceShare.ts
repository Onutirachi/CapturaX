// Placas de captura costumam aceitar um dono só. Quem abre um dispositivo (o preview ou a
// gravação/transmissão) registra a trilha aqui; quem precisar do mesmo dispositivo depois usa uma
// cópia dela (`clone()`) em vez de tentar abrir de novo. A cópia segue viva mesmo que a original
// seja fechada, então cada lado pode parar sem derrubar o outro.

const held = new Map<string, Set<MediaStreamTrack>>();

export function holdDevice(deviceId: string, track: MediaStreamTrack) {
    if (!held.has(deviceId)) held.set(deviceId, new Set());
    held.get(deviceId)!.add(track);
}

export function releaseDevice(deviceId: string, track: MediaStreamTrack) {
    held.get(deviceId)?.delete(track);
}

/** Uma trilha viva do dispositivo, se alguém já o tem aberto. */
export function sharedDeviceTrack(deviceId: string): MediaStreamTrack | null {
    for (const track of held.get(deviceId) ?? []) if (track.readyState === "live") return track;
    return null;
}

/** `deviceId` de uma trilha de câmera/placa (vazio para telas/janelas geradas). */
export const trackDeviceId = (track: MediaStreamTrack | null) => (track?.getSettings().deviceId as string | undefined) ?? "";
