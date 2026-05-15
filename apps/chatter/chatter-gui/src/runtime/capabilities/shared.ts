export function disposeOnce(unlisten: () => void): () => void {
  let disposed = false;

  return () => {
    if (disposed) return;
    disposed = true;
    unlisten();
  };
}

export function singlePathSelection(selected: string | string[] | null): string | null {
  if (!selected) return null;
  return Array.isArray(selected) ? selected[0] ?? null : selected;
}
