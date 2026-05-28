// Right-aligned eyebrow-label + mono-value stat. Used in JobsHeader.

interface Props {
  label: string;
  value: string;
  mono?: boolean;
}

export default function Stat({ label, value, mono }: Props) {
  return (
    <div style={{ textAlign: "right" }}>
      <div className="ba-eyebrow">{label}</div>
      <div
        style={{
          fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
          fontSize: "var(--fs-md)",
          fontWeight: 500,
          fontVariantNumeric: "tabular-nums",
          lineHeight: 1.1,
          marginTop: 2,
        }}
      >
        {value}
      </div>
    </div>
  );
}
