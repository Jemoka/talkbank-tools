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

export default function TranscribePanel({ batchId, config }: Props) {
  const { capabilities, dispatch } = useStore();
  const asrEngines =
    capabilities?.backends_by_task["ASR"] ??
    ["WhisperXBackend", "RevAI", "Qwen3AsrBackend"];

  const set = (patch: VerbConfig) =>
    dispatch({
      type: "VERB_CONFIG_CHANGED",
      batchId,
      verb: "transcribe",
      patch,
    });

  const lang = (config.lang as string) ?? "eng";
  const speakers = (config.speakers as number) ?? 2;
  const engine = (config.engine as string) ?? "WhisperXBackend";
  const diarize = (config.diarize as boolean) ?? true;

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
    </div>
  );
}
