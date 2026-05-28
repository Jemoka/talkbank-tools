// compare panel — diff a transcript against a gold reference.

import { useStore, type VerbConfig } from "../store";
import FieldRow from "../components/FieldRow";
import PathInput from "../components/PathInput";

interface Props {
  batchId: string;
  config: VerbConfig;
}

export default function ComparePanel({ batchId, config }: Props) {
  const { dispatch } = useStore();
  const set = (patch: VerbConfig) =>
    dispatch({
      type: "VERB_CONFIG_CHANGED",
      batchId,
      verb: "compare",
      patch,
    });

  const lang = (config.lang as string) ?? "eng";
  const gold = (config.gold_path as string) ?? "";

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
      <FieldRow label="gold reference">
        <PathInput
          value={gold}
          onChange={(p) => set({ gold_path: p })}
          directory
        />
      </FieldRow>
    </div>
  );
}
