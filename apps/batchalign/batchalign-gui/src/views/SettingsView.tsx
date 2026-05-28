// Full-screen settings view. Left nav lists sections; right body shows
// the currently-selected section.

import { useState } from "react";
import { useStore } from "../store";
import FieldRow from "../components/FieldRow";
import Toggle from "../components/Toggle";
import RadioGroup from "../components/RadioGroup";

const SECTIONS = [
  { id: "general", label: "general" },
  { id: "asr", label: "asr engines" },
  { id: "defaults", label: "pipeline defaults" },
  { id: "paths", label: "paths" },
  { id: "about", label: "about" },
] as const;

type SectionId = (typeof SECTIONS)[number]["id"];

export default function SettingsView() {
  const [active, setActive] = useState<SectionId>("general");
  return (
    <div
      style={{
        flex: 1,
        display: "flex",
        minHeight: 0,
        background: "var(--bg)",
      }}
    >
      <div
        style={{
          width: 220,
          flexShrink: 0,
          borderRight: "var(--hairline)",
          padding: "14px 0",
        }}
      >
        {SECTIONS.map((s) => (
          <div
            key={s.id}
            onClick={() => setActive(s.id)}
            style={{
              padding: "7px 20px",
              cursor: "pointer",
              fontSize: "var(--fs-sm)",
              color: s.id === active ? "var(--dark-blue)" : "var(--fg)",
              background: s.id === active ? "var(--bg-sunken)" : "transparent",
              borderLeft:
                s.id === active
                  ? "2px solid var(--dark-blue)"
                  : "2px solid transparent",
              fontWeight: s.id === active ? 600 : 500,
            }}
          >
            {s.label}
          </div>
        ))}
      </div>
      <div className="ba-scroll" style={{ flex: 1, minHeight: 0 }}>
        <div style={{ maxWidth: 720, padding: "24px 32px 40px" }}>
          {active === "general" && <GeneralSection />}
          {active === "asr" && <AsrSection />}
          {active === "defaults" && <DefaultsSection />}
          {active === "paths" && <PathsSection />}
          {active === "about" && <AboutSection />}
        </div>
      </div>
    </div>
  );
}

function Section({
  eyebrow,
  title,
  children,
}: {
  eyebrow: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ marginBottom: 32 }}>
      <div className="ba-eyebrow">{eyebrow}</div>
      <h2
        style={{
          margin: "2px 0 14px",
          fontSize: "var(--fs-xl)",
          fontWeight: 500,
        }}
      >
        {title}
      </h2>
      {children}
    </div>
  );
}

function GeneralSection() {
  const { settings, dispatch } = useStore();
  const set = (patch: Partial<typeof settings>) =>
    dispatch({ type: "SETTINGS_UPDATED", patch });
  return (
    <Section eyebrow="general" title="settings">
      <FieldRow label="default workers">
        <input
          className="ba-input ba-num"
          type="number"
          value={settings.defaultWorkers}
          onChange={(e) => set({ defaultWorkers: Number(e.target.value) })}
          style={{ width: 80 }}
        />
      </FieldRow>
      <FieldRow label="force cpu">
        <Toggle on={settings.forceCpu} onChange={(v) => set({ forceCpu: v })} />
      </FieldRow>
      <FieldRow label="memory guard">
        <Toggle
          on={settings.memoryGuard}
          onChange={(v) => set({ memoryGuard: v })}
        />
      </FieldRow>
      <FieldRow label="adaptive workers">
        <Toggle
          on={settings.adaptiveWorkers}
          onChange={(v) => set({ adaptiveWorkers: v })}
        />
      </FieldRow>
      <FieldRow label="verbosity">
        <RadioGroup
          value={settings.verbosity}
          options={[
            ["quiet", "quiet"],
            ["v", "-v"],
            ["vv", "-vv"],
          ]}
          onChange={(v) =>
            set({ verbosity: v as typeof settings.verbosity })
          }
        />
      </FieldRow>
    </Section>
  );
}

function AsrSection() {
  const { capabilities } = useStore();
  const asrEngines =
    capabilities?.backends_by_task["ASR"] ?? ["WhisperXBackend", "RevAI"];
  return (
    <Section eyebrow="asr engines" title="speech-to-text">
      <FieldRow label="available engines">
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          {asrEngines.map((e) => (
            <span key={e} className="ba-mono" style={{ fontSize: "var(--fs-sm)" }}>
              {e}
            </span>
          ))}
        </div>
      </FieldRow>
      <FieldRow
        label="rev.ai api key"
        sub="stored locally; configure via ~/.batchalign.ini for now"
      >
        <input
          className="ba-input ba-input--mono"
          type="password"
          placeholder="not set"
          defaultValue=""
          style={{ width: 280 }}
        />
      </FieldRow>
    </Section>
  );
}

function DefaultsSection() {
  return (
    <Section eyebrow="pipeline defaults" title="defaults">
      <FieldRow label="default chain">
        <span className="ba-mono" style={{ fontSize: "var(--fs-sm)" }}>
          transcribe › morphotag
        </span>
      </FieldRow>
    </Section>
  );
}

function PathsSection() {
  return (
    <Section eyebrow="paths" title="cache & config">
      <FieldRow
        label="cache directory"
        sub="managed by the CLI; set via XDG_CACHE_HOME"
      >
        <span className="ba-mono" style={{ fontSize: "var(--fs-sm)" }}>
          $XDG_CACHE_HOME/batchalign-api
        </span>
      </FieldRow>
    </Section>
  );
}

function AboutSection() {
  const { daemon, capabilities } = useStore();
  return (
    <Section eyebrow="about" title="batchalign">
      <FieldRow label="daemon">
        <span className="ba-mono" style={{ fontSize: "var(--fs-sm)" }}>
          {daemon.ready ? `127.0.0.1:${daemon.port}` : "starting…"}
        </span>
      </FieldRow>
      <FieldRow label="api version">
        <span className="ba-mono" style={{ fontSize: "var(--fs-sm)" }}>
          {capabilities?.api_version ?? "—"}
        </span>
      </FieldRow>
    </Section>
  );
}
