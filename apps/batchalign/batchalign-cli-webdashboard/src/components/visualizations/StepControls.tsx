import { useCallback, useEffect, useRef, useState } from "react";

interface StepControlsProps {
  /** Total number of steps. */
  total: number;
  /** Current step index (0-based). */
  current: number;
  /** Called when the step changes. */
  onStep: (step: number) => void;
}

/**
 * Play / Pause / Step / Skip / Speed controls for stepping through
 * algorithm steps.
 */
export function StepControls({ total, current, onStep }: StepControlsProps) {
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const rafRef = useRef<number | null>(null);
  const lastTimeRef = useRef(0);

  const intervalMs = Math.max(20, 200 / speed);

  const stop = useCallback(() => {
    setPlaying(false);
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
  }, []);

  useEffect(() => {
    if (!playing) return;
    if (current >= total - 1) {
      stop();
      return;
    }

    const tick = (time: number) => {
      if (time - lastTimeRef.current >= intervalMs) {
        lastTimeRef.current = time;
        onStep(Math.min(current + 1, total - 1));
      }
      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
    };
  }, [playing, current, total, intervalMs, onStep, stop]);

  return (
    <div className="flex items-center gap-2 text-sm select-none">
      <button
        className="px-2 py-1 rounded bg-gray-100 hover:bg-gray-200 disabled:opacity-40"
        onClick={() => onStep(0)}
        disabled={current === 0}
        title="Reset"
      >
        |&lt;
      </button>
      <button
        className="px-2 py-1 rounded bg-gray-100 hover:bg-gray-200 disabled:opacity-40"
        onClick={() => onStep(Math.max(0, current - 1))}
        disabled={current === 0}
        title="Step back"
      >
        &lt;
      </button>
      <button
        className="px-3 py-1 rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-40"
        onClick={() => (playing ? stop() : setPlaying(true))}
        disabled={total === 0}
      >
        {playing ? "Pause" : "Play"}
      </button>
      <button
        className="px-2 py-1 rounded bg-gray-100 hover:bg-gray-200 disabled:opacity-40"
        onClick={() => onStep(Math.min(total - 1, current + 1))}
        disabled={current >= total - 1}
        title="Step forward"
      >
        &gt;
      </button>
      <button
        className="px-2 py-1 rounded bg-gray-100 hover:bg-gray-200 disabled:opacity-40"
        onClick={() => {
          stop();
          onStep(total - 1);
        }}
        disabled={current >= total - 1}
        title="Skip to end"
      >
        &gt;|
      </button>

      <span className="text-gray-400 mx-1">|</span>

      <span className="text-xs text-gray-500">Speed:</span>
      {[1, 2, 5, 10].map((s) => (
        <button
          key={s}
          className={`px-1.5 py-0.5 rounded text-xs ${
            speed === s
              ? "bg-blue-100 text-blue-700 font-medium"
              : "bg-gray-50 text-gray-500 hover:bg-gray-100"
          }`}
          onClick={() => setSpeed(s)}
        >
          {s}x
        </button>
      ))}

      <span className="ml-2 text-xs text-gray-400 tabular-nums">
        {current + 1} / {total}
      </span>
    </div>
  );
}
