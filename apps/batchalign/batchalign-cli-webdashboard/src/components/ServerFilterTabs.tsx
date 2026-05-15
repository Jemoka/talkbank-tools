import { useStore } from "../state";

interface Props {
  selected: string | null;
  onSelect: (server: string | null) => void;
}

/** Horizontal tabs: [All] [net] [worker-machine] [brian] ... */
export function ServerFilterTabs({ selected, onSelect }: Props) {
  const wsMap = useStore((s) => s.wsConnectedMap);
  const servers = [...wsMap.keys()].sort();

  // Only show tabs when multiple servers are connected
  if (servers.length <= 1) return null;

  return (
    <div className="flex items-center gap-1 mb-4">
      <Tab
        label="All"
        active={selected === null}
        onClick={() => onSelect(null)}
      />
      {servers.map((s) => (
        <Tab
          key={s}
          label={s}
          active={selected === s}
          connected={wsMap.get(s) ?? false}
          onClick={() => onSelect(s)}
        />
      ))}
    </div>
  );
}

function Tab({
  label,
  active,
  connected,
  onClick,
}: {
  label: string;
  active: boolean;
  connected?: boolean;
  onClick: () => void;
}) {
  const base =
    "px-3 py-1 text-xs rounded-full cursor-pointer transition-colors";
  const style = active
    ? "bg-zinc-800 text-white"
    : "bg-zinc-100 text-zinc-600 hover:bg-zinc-200";

  return (
    <button className={`${base} ${style}`} onClick={onClick}>
      {connected !== undefined && (
        <span
          className={`inline-block w-1.5 h-1.5 rounded-full mr-1.5 ${
            connected ? "bg-emerald-400" : "bg-red-400"
          }`}
        />
      )}
      {label}
    </button>
  );
}
