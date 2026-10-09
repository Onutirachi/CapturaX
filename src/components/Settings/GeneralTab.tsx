import { useContext } from "preact/hooks";
import { NativeContext } from "../../context/NativeContext";
import { LANGUAGES, systemLanguage, useT } from "../../i18n";
import { Row, Section, Select } from "./controls";

const SYSTEM = "system";

export default function GeneralTab() {
    const t = useT();
    const nctx = useContext(NativeContext)!;
    const language = nctx.config!.language;
    const system = LANGUAGES.find((l) => l.code === systemLanguage())?.name ?? "English";

    return (
        <div className="flex flex-col gap-6">
            <Section title={t("general.title")}>
                <Row label={t("general.language")} hint={t("general.languageHint")}>
                    <Select
                        value={language ?? SYSTEM}
                        options={[{ value: SYSTEM, label: t("general.languageSystem", { name: system }) }, ...LANGUAGES.map((l) => ({ value: l.code, label: l.name }))]}
                        onChange={(value) => nctx.updateConfig((c) => ({ ...c, language: value === SYSTEM ? null : value }))}
                    />
                </Row>
            </Section>
        </div>
    );
}
