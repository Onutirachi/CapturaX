import { useContext, useEffect, useState } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import { native, errorText, type AppListEntry } from "../../core/native/api";
import { Badge, Button, Note, Section } from "./controls";

const REFRESH_MS = 3000;

function Checkbox({ checked, disabled, onChange }: { checked: boolean; disabled?: boolean; onChange?: (value: boolean) => void }) {
    return <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange?.(e.currentTarget.checked)} className="w-5 h-5 accent-accent cursor-pointer disabled:cursor-not-allowed" />;
}

export default function AppsTab() {
    const nctx = useContext(NativeContext)!;
    const audio = nctx.config!.audio;
    const [apps, setApps] = useState<AppListEntry[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [manual, setManual] = useState("");

    useEffect(() => {
        const load = () =>
            native
                .audioApps()
                .then((list) => (setApps(list), setError(null)))
                .catch((e) => setError(errorText(e)));
        load();
        const interval = window.setInterval(load, REFRESH_MS);
        return () => window.clearInterval(interval);
    }, []);

    const isExcluded = (exe: string) => audio.excludedApps.some((e) => e.toLowerCase() === exe);
    const setIncluded = (exe: string, included: boolean) =>
        nctx.updateConfig((c) => {
            const others = c.audio.excludedApps.filter((e) => e.toLowerCase() !== exe);
            return { ...c, audio: { ...c.audio, excludedApps: included ? others : [...others, exe] } };
        });

    const addManual = () => {
        let exe = manual.trim().toLowerCase();
        if (!exe) return;
        if (!exe.endsWith(".exe")) exe += ".exe";
        nctx.updateConfig((c) => ({
            ...c,
            audio: {
                ...c.audio,
                knownApps: c.audio.knownApps.includes(exe) ? c.audio.knownApps : [...c.audio.knownApps, exe],
                excludedApps: c.audio.excludedApps.some((e) => e.toLowerCase() === exe) ? c.audio.excludedApps : [...c.audio.excludedApps, exe],
            },
        }));
        setManual("");
    };

    // Apps salvos que ainda não apareceram na lista do Rust (ex.: acabaram de ser adicionados).
    const listed = new Set(apps.map((a) => a.exe));
    const extra: AppListEntry[] = [...audio.knownApps, ...audio.excludedApps]
        .map((e) => e.toLowerCase())
        .filter((exe, i, all) => !listed.has(exe) && all.indexOf(exe) === i)
        .map((exe) => ({ exe, name: exe.replace(/\.exe$/, ""), running: false, playing: false, excluded: true }));

    return (
        <div className="flex flex-col gap-6">
            <Section
                title="App audio"
                description="Checked apps go into recordings and streams. Unchecked ones are left out, so whoever is watching doesn't hear them (like your Discord call). Changes apply right away, even while recording.">
                {!audio.recordSystem && (
                    <div className="px-4 py-3">
                        <Note tone="warning">App audio is turned off in the Audio tab, so no app is captured right now.</Note>
                    </div>
                )}
                <div className="flex items-center gap-4 px-4 py-3 opacity-70">
                    <Checkbox checked={false} disabled />
                    <div className="flex-1">
                        <div>CapturaX</div>
                        <div className="text-sm text-(--text-muted)">Always left out, so the preview sound is never recorded twice.</div>
                    </div>
                </div>
                {[...apps, ...extra].map((app) => (
                    <label key={app.exe} className="flex items-center gap-4 px-4 py-3 cursor-pointer hover:bg-normal/30">
                        <Checkbox checked={!isExcluded(app.exe)} onChange={(included) => setIncluded(app.exe, included)} />
                        <div className="flex-1 min-w-0">
                            <div className="truncate">{app.name}</div>
                            <div className="text-sm text-(--text-muted) truncate">{app.exe}</div>
                        </div>
                        {app.playing ? <Badge color="var(--color-accent)">Playing</Badge> : app.running ? <Badge>Open</Badge> : <Badge color="transparent">Not running</Badge>}
                    </label>
                ))}
                <div className="flex items-center gap-3 px-4 py-3">
                    <input
                        value={manual}
                        placeholder="Add an app that isn't open (e.g. discord.exe)"
                        onInput={(e) => setManual(e.currentTarget.value)}
                        onKeyDown={(e) => e.key === "Enter" && addManual()}
                        className="flex-1 h-9 px-3 rounded-lg bg-dark [border:var(--border)] outline-none focus:border-accent"
                    />
                    <Button onClick={addManual} disabled={!manual.trim()}>
                        Add (left out)
                    </Button>
                </div>
            </Section>
            {error && <Note tone="error">Could not list apps: {error}</Note>}
            <Note>
                Windows separates audio per program: two things playing in the same program (like two browser tabs) are included or left out together.
            </Note>
        </div>
    );
}
