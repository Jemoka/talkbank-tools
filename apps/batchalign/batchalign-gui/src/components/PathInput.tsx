// Mono-font path input + a "choose…" button. Wraps Tauri dialog plugin
// (open) so it works in dev and bundle paths identically.

import { open } from "@tauri-apps/plugin-dialog";

interface Props {
  value: string;
  onChange?: (path: string) => void;
  directory?: boolean;
}

export default function PathInput({ value, onChange, directory }: Props) {
  const choose = async () => {
    const selected = await open({
      directory: !!directory,
      multiple: false,
    });
    if (typeof selected === "string") onChange?.(selected);
  };
  return (
    <div style={{ display: "flex", gap: 6 }}>
      <input
        className="ba-input ba-input--mono"
        value={value}
        onChange={(e) => onChange?.(e.target.value)}
        style={{ flex: 1 }}
      />
      <button
        type="button"
        className="ba-btn ba-btn--sm"
        onClick={choose}
        style={{ padding: "3px 10px" }}
      >
        choose…
      </button>
    </div>
  );
}
