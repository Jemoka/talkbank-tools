/** Selectable card for an ASR engine option in the setup wizard.
 *
 * Shows the engine name, description, and a pros/cons list. Visually
 * highlights the selected card with an indigo border.
 */

interface EngineCardProps {
  name: string;
  description: string;
  pros: string[];
  cons: string[];
  selected: boolean;
  onSelect: () => void;
}

export function EngineCard({
  name,
  description,
  pros,
  cons,
  selected,
  onSelect,
}: EngineCardProps) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={`w-full text-left p-4 rounded-lg border-2 transition-all ${
        selected
          ? "border-indigo-500 bg-indigo-50/50"
          : "border-gray-200 hover:border-gray-300"
      }`}
    >
      <div className="flex items-center justify-between">
        <div>
          <div className="text-sm font-semibold text-gray-900">{name}</div>
          <div className="text-xs text-gray-500 mt-0.5">{description}</div>
        </div>
        <div
          className={`w-5 h-5 rounded-full border-2 flex items-center justify-center flex-shrink-0 ${
            selected ? "border-indigo-500" : "border-gray-300"
          }`}
        >
          {selected && (
            <div className="w-2.5 h-2.5 rounded-full bg-indigo-500" />
          )}
        </div>
      </div>

      <div className="mt-3 flex gap-6 text-xs">
        <div>
          {pros.map((p) => (
            <div key={p} className="text-emerald-600 mt-0.5">
              + {p}
            </div>
          ))}
        </div>
        <div>
          {cons.map((c) => (
            <div key={c} className="text-gray-400 mt-0.5">
              - {c}
            </div>
          ))}
        </div>
      </div>
    </button>
  );
}
