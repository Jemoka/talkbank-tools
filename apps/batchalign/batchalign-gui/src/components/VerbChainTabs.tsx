// Underlined-active tabs for the pipeline verbs. Reads `pipeline` from
// the active batch; renders chevrons between verbs as gray text, the
// active verb gets the slate underline + dark-blue color. Step
// management buttons ("remove <verb>" / "+ add step") sit on the right.

import { Fragment, useState } from "react";
import { useStore, type VerbStep } from "../store";

const AVAILABLE_VERBS: VerbStep[] = [
  "transcribe",
  "align",
  "morphotag",
  "translate",
  "compare",
];

interface Props {
  selected: VerbStep;
  onSelect: (v: VerbStep) => void;
}

export default function VerbChainTabs({ selected, onSelect }: Props) {
  const { activeBatchId, batches, dispatch } = useStore();
  const batch = activeBatchId ? batches[activeBatchId] : null;
  if (!batch) return null;
  const chain = batch.pipeline;
  const canRemove = chain.length > 1;
  const unused = AVAILABLE_VERBS.filter((v) => !chain.includes(v));

  const onRemove = () => {
    const next = chain.filter((x) => x !== selected);
    dispatch({
      type: "PIPELINE_CHANGED",
      batchId: batch.id,
      pipeline: next,
    });
    onSelect(next[0]);
  };

  const onAdd = (v: VerbStep) => {
    dispatch({
      type: "PIPELINE_CHANGED",
      batchId: batch.id,
      pipeline: [...chain, v],
    });
    onSelect(v);
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        paddingBottom: 10,
        borderBottom: "var(--hairline)",
        marginBottom: 14,
      }}
    >
      {chain.map((v, i) => {
        const isActive = selected === v;
        return (
          <Fragment key={v}>
            {i > 0 && (
              <span
                style={{
                  color: "var(--gray-3)",
                  fontSize: 14,
                  padding: "4px 10px",
                  marginBottom: -12,
                  lineHeight: 1,
                  userSelect: "none",
                }}
              >
                ›
              </span>
            )}
            <button
              type="button"
              onClick={() => onSelect(v)}
              style={{
                background: "transparent",
                border: 0,
                padding: "4px 2px",
                cursor: "pointer",
                fontSize: "var(--fs-md)",
                fontWeight: 600,
                color: isActive ? "var(--dark-blue)" : "var(--fg-muted)",
                borderBottom: isActive
                  ? "2px solid var(--dark-blue)"
                  : "2px solid transparent",
                marginBottom: -12,
                letterSpacing: "-0.005em",
              }}
            >
              {v}
            </button>
          </Fragment>
        );
      })}
      <span style={{ flex: 1 }} />
      <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
        {canRemove && (
          <button
            type="button"
            onClick={onRemove}
            className="ba-btn ba-btn--sm"
            style={{ padding: "3px 9px", fontSize: "var(--fs-xs)" }}
          >
            remove {selected}
          </button>
        )}
        <AddVerb unused={unused} onAdd={onAdd} />
      </div>
    </div>
  );
}

function AddVerb({
  unused,
  onAdd,
}: {
  unused: VerbStep[];
  onAdd: (v: VerbStep) => void;
}) {
  const [open, setOpen] = useState(false);
  if (unused.length === 0) return null;
  return (
    <div style={{ position: "relative" }}>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="ba-btn ba-btn--sm"
        style={{ padding: "3px 9px", fontSize: "var(--fs-xs)" }}
      >
        + add step
      </button>
      {open && (
        <div
          style={{
            position: "absolute",
            top: "100%",
            right: 0,
            marginTop: 4,
            background: "var(--surface)",
            border: "var(--rule)",
            borderRadius: "var(--r-1)",
            minWidth: 160,
            zIndex: 10,
            boxShadow: "0 2px 8px rgba(0,0,0,0.08)",
          }}
        >
          {unused.map((v) => (
            <div
              key={v}
              onClick={() => {
                onAdd(v);
                setOpen(false);
              }}
              style={{
                padding: "6px 12px",
                fontSize: "var(--fs-sm)",
                cursor: "pointer",
                fontFamily: "var(--font-mono)",
              }}
              onMouseEnter={(e) =>
                (e.currentTarget.style.background = "var(--bg-sunken)")
              }
              onMouseLeave={(e) =>
                (e.currentTarget.style.background = "transparent")
              }
            >
              {v}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
