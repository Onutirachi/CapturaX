import { useContext, useState } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import { native, errorText, type StreamConfig, type UpnpStatus } from "../../core/native/api";
import { Badge, Button, CopyField, Note, NumberInput, Row, Section, Select, Toggle } from "./controls";

function formatWatching(seconds: number) {
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    return minutes < 60 ? `${minutes} min` : `${Math.floor(minutes / 60)} h ${minutes % 60} min`;
}

function UpnpText({ status }: { status: UpnpStatus }) {
    switch (status.state) {
        case "disabled":
            return <>Off: open the port on the router yourself.</>;
        case "searching":
            return <>Asking the router to open the port…</>;
        case "mapped":
            return status.cgnat ? (
                <span className="text-[goldenrod]">The router opened the port, but your internet provider uses CGNAT ({status.externalIp}): the link won't work from outside.</span>
            ) : (
                <>Port opened on the router (public IP {status.externalIp}).</>
            );
        case "failed":
            return <span className="text-[goldenrod]">{status.reason}. Open the port manually on the router, or enable UPnP there.</span>;
    }
}

export default function StreamingTab() {
    const nctx = useContext(NativeContext)!;
    const stream = nctx.config!.stream;
    const server = nctx.server;
    const set = (patch: Partial<StreamConfig>) => nctx.updateConfig((c) => ({ ...c, stream: { ...c.stream, ...patch } }));
    const [firewallError, setFirewallError] = useState<string | null>(null);
    const [fixing, setFixing] = useState(false);

    const live = nctx.streaming === "active";
    const uploadMbps = ((stream.bitrateKbps + stream.audioBitrateKbps) * stream.maxViewers) / 1000;
    const cgnat = server?.upnp.state === "mapped" && server.upnp.cgnat;

    const fixFirewall = async () => {
        setFixing(true);
        setFirewallError(null);
        await native.firewallFix().catch((e) => setFirewallError(errorText(e)));
        setFixing(false);
    };

    return (
        <div className="flex flex-col gap-6">
            <Section title="Link" description="Friends watch in any browser. The link stays the same; while you're not live it shows “Offline” and starts by itself when you go live.">
                <Row label="Accept viewers" hint="Keeps the link online on this PC.">
                    <Toggle checked={stream.acceptViewers} onChange={(acceptViewers) => set({ acceptViewers })} />
                </Row>
                <Row label={live ? "You are live" : "Not live"} hint={live ? `${server?.viewers.length ?? 0} watching now` : "Start from here or from the Dock."}>
                    <Button variant={live ? "danger" : "accent"} onClick={() => nctx.toggleStreaming()} disabled={nctx.streaming === "starting" || nctx.streaming === "stopping"}>
                        {live ? "Stop" : "Go live"}
                    </Button>
                </Row>
                {server?.publicUrl && (
                    <Row label="Internet link" hint="Send this to whoever is watching.">
                        <CopyField value={server.publicUrl} />
                    </Row>
                )}
                {server?.localUrl && (
                    <Row label="Local network link" hint="Same Wi-Fi/network as this PC.">
                        <CopyField value={server.localUrl} />
                    </Row>
                )}
                {server?.error && (
                    <div className="px-4 py-3">
                        <Note tone="error">{server.error}</Note>
                    </div>
                )}
            </Section>

            <Section title="Network">
                <Row label="Port" hint="Changing it restarts the link.">
                    <NumberInput value={stream.port} min={1024} max={65535} onChange={(port) => set({ port })} />
                </Row>
                <Row label="Open the port automatically (UPnP)" hint={server ? <UpnpText status={server.upnp} /> : "Applies when the link is online."}>
                    <Toggle checked={stream.upnp} onChange={(upnp) => set({ upnp })} />
                </Row>
                <Row
                    label="Windows Firewall"
                    hint={
                        server?.firewallOk ? (
                            "Allowed (only CapturaX, only incoming TCP)."
                        ) : (
                            <>Not allowed yet. Fixing asks for administrator permission once.{firewallError && <span className="text-bad"> {firewallError}</span>}</>
                        )
                    }>
                    {server?.firewallOk ? <Badge color="var(--color-accent)">OK</Badge> : <Button onClick={fixFirewall} disabled={fixing}>{fixing ? "Waiting…" : "Fix firewall"}</Button>}
                </Row>
            </Section>

            <Section title="Quality" description={`Your upload needs about ${uploadMbps.toFixed(1)} Mbps with ${stream.maxViewers} viewers (each one receives the full stream).`}>
                <Row label="Resolution">
                    <Select
                        value={String(stream.height)}
                        onChange={(v) => set({ height: Number(v), width: Math.round((Number(v) * 16) / 9) })}
                        width="w-40"
                        options={[480, 720, 1080, 1440].map((h) => ({ value: String(h), label: `${h}p` }))}
                    />
                </Row>
                <Row label="Frame rate">
                    <Select value={String(stream.fps)} onChange={(v) => set({ fps: Number(v) })} width="w-40" options={[30, 60].map((f) => ({ value: String(f), label: `${f} FPS` }))} />
                </Row>
                <Row label="Video bitrate" hint="3 Mbps is good for 720p30; 6 Mbps for 1080p60.">
                    <NumberInput value={stream.bitrateKbps / 1000} min={0.5} max={50} step={0.5} suffix="Mbps" onChange={(mbps) => set({ bitrateKbps: Math.round(mbps * 1000) })} />
                </Row>
                <Row label="Max viewers">
                    <NumberInput value={stream.maxViewers} min={1} max={100} onChange={(maxViewers) => set({ maxViewers })} />
                </Row>
            </Section>

            <Section title="Viewers" description="Blocking stops an address from opening the link.">
                {(server?.viewers ?? []).length === 0 && <Row label={<span className="text-(--text-muted)">Nobody watching right now.</span>} />}
                {(server?.viewers ?? []).map((viewer) => (
                    <Row key={viewer.ip} label={viewer.ip} hint={`Watching for ${formatWatching(viewer.watchingSecs)}`}>
                        <Button variant="danger" onClick={() => set({ blockedIps: [...stream.blockedIps, viewer.ip] })}>
                            Block
                        </Button>
                    </Row>
                ))}
                {stream.blockedIps.map((ip) => (
                    <Row key={ip} label={<span className="text-(--text-muted)">{ip}</span>} hint="Blocked">
                        <Button onClick={() => set({ blockedIps: stream.blockedIps.filter((b) => b !== ip) })}>Unblock</Button>
                    </Row>
                ))}
            </Section>

            <Note tone={cgnat ? "warning" : "info"}>
                <p>
                    <b>Same link every time:</b> home internet usually changes its public IP from time to time. A free DDNS (DuckDNS, No-IP, or the one in
                    your router) gives you a fixed name like <code>myname.duckdns.org:{stream.port}</code>.
                </p>
                <p className="mt-2">
                    <b>Behind CGNAT or can't open ports?</b> Ask your provider for a public IP, or use a tunnel such as Cloudflare Tunnel or Tailscale
                    Funnel pointing to <code>http://localhost:{stream.port}</code> (they also add HTTPS).
                </p>
            </Note>
        </div>
    );
}
