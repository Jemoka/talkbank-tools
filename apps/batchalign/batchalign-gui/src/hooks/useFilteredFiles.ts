// Filters the active batch's discovered files down to the subset the
// first pipeline verb would actually pick up. Mirrors the daemon's
// `InputKind` classification (see python/batchalign/inputs.py and the
// recipe signatures in python/batchalign/recipes.py):
//
//   - `transcribe`            → media inputs only
//   - `morphotag`, `translate`→ chat (.cha) inputs only
//   - `align`, `compare`      → paired stems (both audio AND chat
//                                with the same stem must exist)
//
// An empty pipeline yields an empty list. The BatchView right-pane
// uses that to decide between the file table and a silhouette
// placeholder.

import { useStore, type FileRow, type VerbStep } from "../store";

function requiredKind(verb: VerbStep): "media" | "chat" | "paired" {
  switch (verb) {
    case "transcribe":
      return "media";
    case "morphotag":
    case "translate":
      return "chat";
    case "align":
    case "compare":
      return "paired";
  }
}

export function filterFilesForVerb(
  files: Record<string, FileRow>,
  fileOrder: string[],
  verb: VerbStep,
): string[] {
  const kind = requiredKind(verb);
  if (kind === "media") {
    return fileOrder.filter((id) => files[id]?.kind === "media");
  }
  if (kind === "chat") {
    return fileOrder.filter((id) => files[id]?.kind === "chat");
  }
  // paired: keep stems that have both audio and chat companions.
  const stemKinds = new Map<string, Set<string>>();
  for (const id of fileOrder) {
    const f = files[id];
    if (!f) continue;
    const set = stemKinds.get(f.stem) ?? new Set<string>();
    set.add(f.kind);
    stemKinds.set(f.stem, set);
  }
  return fileOrder.filter((id) => {
    const f = files[id];
    if (!f) return false;
    const kinds = stemKinds.get(f.stem);
    return !!kinds && kinds.has("media") && kinds.has("chat");
  });
}

/** Returns the IDs of files the daemon would actually process given
 *  the active batch's pipeline. Empty when there is no batch or no
 *  pipeline steps. */
export function useFilteredFiles(): string[] {
  const { activeBatchId, batches } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  if (!batch) return [];
  const firstVerb = batch.pipeline[0];
  if (!firstVerb) return [];
  return filterFilesForVerb(batch.files, batch.fileOrder, firstVerb);
}
