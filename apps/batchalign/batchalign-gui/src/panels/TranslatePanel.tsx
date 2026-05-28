// translate panel — engine choice (Google Translate / vLLM / NLLB /
// Tencent TMT / Aliyun TMT) plus target language.

import { useStore, type VerbConfig } from "../store";
import FieldRow from "../components/FieldRow";
import RadioGroup from "../components/RadioGroup";

interface Props {
  batchId: string;
  config: VerbConfig;
}

const PRETTY_TR: Record<string, string> = {
  GoogleTranslateBackend: "Google Translate (cloud)",
  VllmTranslateBackend: "vLLM (local)",
  NllbTranslateBackend: "NLLB-1.3B (local)",
  TencentTmtBackend: "Tencent TMT (cloud)",
  AliyunTranslateBackend: "Aliyun MT (cloud)",
};

export default function TranslatePanel({ batchId, config }: Props) {
  const { capabilities, dispatch } = useStore();
  const trEngines =
    capabilities?.backends_by_task["Translate"] ??
    ["GoogleTranslateBackend", "NllbTranslateBackend"];

  const set = (patch: VerbConfig) =>
    dispatch({
      type: "VERB_CONFIG_CHANGED",
      batchId,
      verb: "translate",
      patch,
    });

  const engine = (config.engine as string) ?? "GoogleTranslateBackend";
  const target = (config.target as string) ?? "eng";

  return (
    <div>
      <FieldRow label="engine">
        <RadioGroup
          value={engine}
          options={trEngines.map((e) => [
            e,
            PRETTY_TR[e] ?? e.replace(/Backend$/, ""),
          ])}
          onChange={(v) => set({ engine: v })}
        />
      </FieldRow>
      <FieldRow label="target language">
        <input
          className="ba-input"
          value={target}
          onChange={(e) => set({ target: e.target.value })}
          style={{ width: 90 }}
        />
      </FieldRow>
    </div>
  );
}
