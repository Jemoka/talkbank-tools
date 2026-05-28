// align panel — forced-alignment backend choice.

import { useStore, type VerbConfig } from "../store";
import FieldRow from "../components/FieldRow";
import RadioGroup from "../components/RadioGroup";
import Toggle from "../components/Toggle";

interface Props {
  batchId: string;
  config: VerbConfig;
}

const PRETTY_FA: Record<string, string> = {
  Wav2Vec2FaBackend: "Wav2Vec2 (english)",
  WhisperFaBackend: "Whisper FA (other)",
  WhisperXFaBackend: "WhisperX FA (other)",
};

export default function AlignPanel({ batchId, config }: Props) {
  const { capabilities, dispatch } = useStore();
  const faEngines =
    capabilities?.backends_by_task["FA"] ??
    ["Wav2Vec2FaBackend", "WhisperFaBackend"];

  const set = (patch: VerbConfig) =>
    dispatch({
      type: "VERB_CONFIG_CHANGED",
      batchId,
      verb: "align",
      patch,
    });

  const aligner = (config.aligner as string) ?? "Wav2Vec2FaBackend";
  const writeWor = (config.write_wor as boolean) ?? true;

  return (
    <div>
      <FieldRow label="aligner">
        <RadioGroup
          value={aligner}
          options={faEngines.map((e) => [
            e,
            PRETTY_FA[e] ?? e.replace(/Backend$/, ""),
          ])}
          onChange={(v) => set({ aligner: v })}
        />
      </FieldRow>
      <FieldRow label="write %wor tier">
        <Toggle on={writeWor} onChange={(on) => set({ write_wor: on })} />
      </FieldRow>
    </div>
  );
}
