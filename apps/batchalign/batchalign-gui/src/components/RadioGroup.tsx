// Vertical radio group. Used by panels for engine selection etc.

interface Props {
  value: string;
  options: Array<[string, string]>;
  onChange?: (v: string) => void;
}

export default function RadioGroup({ value, options, onChange }: Props) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
      {options.map(([v, label]) => (
        <label
          key={v}
          onClick={() => onChange?.(v)}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            fontSize: "var(--fs-sm)",
            cursor: onChange ? "pointer" : "default",
          }}
        >
          <span
            style={{
              width: 12,
              height: 12,
              borderRadius: "50%",
              border: "1px solid var(--gray-3)",
              background:
                value === v ? "var(--dark-blue)" : "var(--surface)",
              boxShadow:
                value === v ? "inset 0 0 0 2.5px var(--surface)" : "none",
              flexShrink: 0,
            }}
          />
          {label}
        </label>
      ))}
    </div>
  );
}
