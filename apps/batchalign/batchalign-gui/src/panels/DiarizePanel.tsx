import { useStore, type VerbConfig } from "../store";
import FieldRow from "../components/FieldRow";
import RadioGroup from "../components/RadioGroup";

interface Props {
  batchId: string;
  config: VerbConfig;
}

const PRETTY: Record<string, string> = {
  PyannoteAIBackend: "pyannoteAI (cloud)",
  PyannoteBackend: "Pyannote (local)",
};

export default function DiarizePanel({ batchId, config }: Props) {
  const { capabilities, dispatch } = useStore();
  const engines = (
    capabilities?.backends_by_task["Speaker"] ??
    ["PyannoteAIBackend", "PyannoteBackend"]
  ).filter((name) => name.startsWith("Pyannote"));
  const engine = (config.engine as string) ?? "PyannoteAIBackend";
  const speakers = (config.speakers as number) ?? 0;
  const set = (patch: VerbConfig) =>
    dispatch({ type: "VERB_CONFIG_CHANGED", batchId, verb: "diarize", patch });

  return (
    <div>
      <FieldRow label="engine">
        <RadioGroup
          value={engine}
          options={engines.map((name) => [name, PRETTY[name] ?? name])}
          onChange={(value) => set({ engine: value })}
        />
      </FieldRow>
      <FieldRow label="speakers (0 = auto)">
        <input
          className="ba-input ba-num"
          type="number"
          min={0}
          value={speakers}
          onChange={(event) => set({ speakers: Number(event.target.value) })}
          style={{ width: 70 }}
        />
      </FieldRow>
    </div>
  );
}
