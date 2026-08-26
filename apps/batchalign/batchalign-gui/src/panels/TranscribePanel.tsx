// transcribe panel — picks language, engine, speaker count, diarize.
// Engines list comes from the live daemon's `/capabilities` ASR set.

import { useStore, type VerbConfig } from "../store";
import FieldRow from "../components/FieldRow";
import RadioGroup from "../components/RadioGroup";
import Toggle from "../components/Toggle";

interface Props {
  batchId: string;
  config: VerbConfig;
}

const PRETTY_ASR: Record<string, string> = {
  WhisperBackend: "Whisper (local)",
  WhisperXBackend: "WhisperX (local)",
  ChatWhisperBackend: "Chat-Whisper (local)",
  OpenAIWhisperBackend: "OpenAI Whisper (local)",
  VllmAsrBackend: "vLLM Whisper (local)",
  RevAI: "Rev.AI (cloud)",
  TencentAsrBackend: "Tencent Cloud ASR",
  AliyunAsrBackend: "Aliyun Cloud ASR",
  FunAsrBackend: "FunASR / SenseVoice",
  FunAudioBackend: "Paraformer-zh",
  Qwen3AsrBackend: "Qwen3-ASR",
  QwenAsrBackend: "Qwen-ASR",
};

const PRETTY_SPEAKER: Record<string, string> = {
  PyannoteAIBackend: "pyannoteAI (cloud)",
  PyannoteBackend: "Pyannote (local)",
};

export default function TranscribePanel({ batchId, config }: Props) {
  const { capabilities, dispatch } = useStore();
  const asrEngines =
    capabilities?.backends_by_task["ASR"] ??
    ["WhisperBackend", "RevAI", "Qwen3AsrBackend"];
  const speakerEngines = (
    capabilities?.backends_by_task["Speaker"] ??
    ["PyannoteAIBackend", "PyannoteBackend"]
  ).filter((name) => name.startsWith("Pyannote"));

  const set = (patch: VerbConfig) =>
    dispatch({
      type: "VERB_CONFIG_CHANGED",
      batchId,
      verb: "transcribe",
      patch,
    });

  const lang = (config.lang as string) ?? "eng";
  const speakers = (config.speakers as number) ?? 2;
  const engine = (config.engine as string) ?? "WhisperBackend";
  const diarize = (config.diarize as boolean) ?? true;
  const nativeSpeaker = engine === "RevAI" || engine === "GoogleGenAIBackend";
  const diarizeEngine =
    (config.diarize_engine as string) ?? "PyannoteAIBackend";

  return (
    <div>
      <FieldRow label="language">
        <input
          className="ba-input"
          value={lang}
          onChange={(e) => set({ lang: e.target.value })}
          style={{ width: 90 }}
        />
      </FieldRow>
      <FieldRow label="speakers">
        <input
          className="ba-input ba-num"
          type="number"
          value={speakers}
          onChange={(e) => set({ speakers: Number(e.target.value) })}
          style={{ width: 70 }}
        />
      </FieldRow>
      <FieldRow label="engine">
        <RadioGroup
          value={engine}
          options={asrEngines.map((e) => [
            e,
            PRETTY_ASR[e] ?? e.replace(/Backend$/, ""),
          ])}
          onChange={(v) => set({ engine: v })}
        />
      </FieldRow>
      <FieldRow label="diarize">
        <Toggle on={diarize} onChange={(on) => set({ diarize: on })} />
      </FieldRow>
      {diarize && !nativeSpeaker && (
        <FieldRow label="diarizer">
          <RadioGroup
            value={diarizeEngine}
            options={speakerEngines.map((name) => [
              name,
              PRETTY_SPEAKER[name] ?? name,
            ])}
            onChange={(value) => set({ diarize_engine: value })}
          />
        </FieldRow>
      )}
      {diarize && nativeSpeaker && (
        <FieldRow label="diarizer">
          <span className="ba-mono">{PRETTY_ASR[engine] ?? engine}</span>
        </FieldRow>
      )}
    </div>
  );
}
