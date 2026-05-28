// verb → panel component. Single map; the PipelineBlock consults this to
// pick which panel to render. Adding a new verb requires (a) extending
// the VerbStep union in store.ts, (b) writing a Panel component, (c)
// adding the verb here.

import type { ComponentType } from "react";
import type { VerbConfig, VerbStep } from "../store";
import TranscribePanel from "./TranscribePanel";
import AlignPanel from "./AlignPanel";
import MorphotagPanel from "./MorphotagPanel";
import TranslatePanel from "./TranslatePanel";
import ComparePanel from "./ComparePanel";

export interface PanelProps {
  batchId: string;
  config: VerbConfig;
}

const REGISTRY: Record<VerbStep, ComponentType<PanelProps>> = {
  transcribe: TranscribePanel,
  align: AlignPanel,
  morphotag: MorphotagPanel,
  translate: TranslatePanel,
  compare: ComparePanel,
};

export function panelFor(verb: VerbStep): ComponentType<PanelProps> {
  return REGISTRY[verb];
}
