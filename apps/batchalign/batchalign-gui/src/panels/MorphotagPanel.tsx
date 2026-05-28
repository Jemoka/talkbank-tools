// morphotag panel — stanza-driven POS+deps over existing .cha files.

import { useStore, type VerbConfig } from "../store";
import FieldRow from "../components/FieldRow";
import Toggle from "../components/Toggle";

interface Props {
  batchId: string;
  config: VerbConfig;
}

export default function MorphotagPanel({ batchId, config }: Props) {
  const { dispatch } = useStore();
  const set = (patch: VerbConfig) =>
    dispatch({
      type: "VERB_CONFIG_CHANGED",
      batchId,
      verb: "morphotag",
      patch,
    });

  const retokenize = (config.retokenize as boolean) ?? true;
  const useCache = (config.use_cache as boolean) ?? true;
  const skipCs = (config.skip_codeswitching as boolean) ?? false;

  return (
    <div>
      <FieldRow label="retokenize">
        <Toggle on={retokenize} onChange={(on) => set({ retokenize: on })} />
      </FieldRow>
      <FieldRow label="use cache">
        <Toggle on={useCache} onChange={(on) => set({ use_cache: on })} />
      </FieldRow>
      <FieldRow label="skip code-switching">
        <Toggle
          on={skipCs}
          onChange={(on) => set({ skip_codeswitching: on })}
        />
      </FieldRow>
    </div>
  );
}
