// Pill toggle. Controlled; parent owns the boolean.

interface Props {
  on: boolean;
  onChange?: (on: boolean) => void;
}

export default function Toggle({ on, onChange }: Props) {
  return (
    <div
      onClick={() => onChange?.(!on)}
      style={{
        width: 28,
        height: 16,
        borderRadius: 999,
        background: on ? "var(--green)" : "var(--gray-2)",
        position: "relative",
        cursor: onChange ? "pointer" : "default",
        flexShrink: 0,
      }}
    >
      <div
        style={{
          position: "absolute",
          top: 1,
          left: on ? 13 : 1,
          width: 14,
          height: 14,
          borderRadius: "50%",
          background: "var(--surface)",
          border: "1px solid rgba(0,0,0,0.06)",
          transition: "left 0.08s ease",
        }}
      />
    </div>
  );
}
