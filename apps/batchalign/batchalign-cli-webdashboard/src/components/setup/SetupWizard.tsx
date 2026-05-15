/** First-time setup wizard for the desktop app.
 *
 * Guides the user through ASR engine configuration on first launch. This
 * mirrors the `batchalign3 setup` / batchalign2 `interactive_setup()` flow
 * in a GUI. Writes to `~/.batchalign.ini` via Tauri config commands.
 *
 * Steps:
 * 1. Welcome — explain what this does
 * 2. Engine selection — Rev.AI (cloud, fast, paid) vs Whisper (local, free)
 * 3. API key — if Rev.AI selected, prompt for key with confirmation
 * 4. Done — summary + "Get Started" button
 */

import { useState } from "react";
import { useDesktopConfig } from "../../desktop/DesktopContext";
import { EngineCard } from "./EngineCard";

type Step = "welcome" | "engine" | "api-key" | "done";
type Engine = "rev" | "whisper";

interface SetupWizardProps {
  /** Called when setup is complete. */
  onComplete: () => void;
}

export function SetupWizard({ onComplete }: SetupWizardProps) {
  const desktopConfig = useDesktopConfig();
  const [step, setStep] = useState<Step>("welcome");
  const [engine, setEngine] = useState<Engine>("whisper");
  const [revKey, setRevKey] = useState("");
  const [revKeyConfirm, setRevKeyConfirm] = useState("");
  const [keyError, setKeyError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function handleSave() {
    if (engine === "rev") {
      if (!revKey.trim()) {
        setKeyError("API key cannot be empty.");
        return;
      }
      if (revKey !== revKeyConfirm) {
        setKeyError("Keys do not match. Please try again.");
        return;
      }
    }

    setSaving(true);
    try {
      await desktopConfig.writeConfig({
        engine,
        rev_key: engine === "rev" ? revKey.trim() : null,
      });
      setStep("done");
    } catch (e) {
      setKeyError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-[var(--bg-page)]">
      <div className="max-w-lg w-full mx-4">
        {/* Welcome */}
        {step === "welcome" && (
          <div className="text-center space-y-6">
            <div>
              <h1 className="text-2xl font-bold text-gray-900">
                Welcome to Batchalign
              </h1>
              <p className="text-gray-600 mt-3 leading-relaxed">
                Thanks for checking out Batchalign! This appears to be your first
                time using the app, so let's do some quick setup.
              </p>
              <p className="text-gray-500 text-sm mt-3 leading-relaxed">
                For speech-to-text tasks, you can choose between Rev.AI (a fast
                cloud service that requires an API key) or Whisper (a free local
                model that runs on your machine). You can always change this later.
              </p>
            </div>
            <button
              onClick={() => setStep("engine")}
              className="px-8 py-3 text-sm font-semibold text-white bg-indigo-600 rounded-lg
                hover:bg-indigo-700 transition-colors"
            >
              Let's Get Started
            </button>
          </div>
        )}

        {/* Engine selection */}
        {step === "engine" && (
          <div className="space-y-5">
            <div>
              <h2 className="text-lg font-semibold text-gray-900">
                Choose your ASR engine
              </h2>
              <p className="text-sm text-gray-500 mt-1">
                This sets your default for speech-to-text. You can override per-job later.
              </p>
            </div>

            <div className="space-y-3">
              <EngineCard
                name="Rev.AI"
                description="Cloud-based, fast and accurate. Requires a paid API key. English only."
                pros={["Faster processing", "Higher accuracy", "No GPU needed"]}
                cons={["English only", "Requires API key", "Costs per minute of audio"]}
                selected={engine === "rev"}
                onSelect={() => setEngine("rev")}
              />
              <EngineCard
                name="Whisper"
                description="Runs locally on your machine. Free, but slower."
                pros={["Free to use", "No API key needed", "Works offline"]}
                cons={["Slower processing", "Requires GPU for best speed", "Downloads ~2 GB of models on first use"]}
                selected={engine === "whisper"}
                onSelect={() => setEngine("whisper")}
              />
            </div>

            <div className="flex justify-between pt-2">
              <button
                onClick={() => setStep("welcome")}
                className="text-sm text-gray-500 hover:text-gray-700 transition-colors"
              >
                Back
              </button>
              <button
                onClick={() => {
                  if (engine === "rev") {
                    setStep("api-key");
                  } else {
                    handleSave();
                  }
                }}
                className="px-6 py-2 text-sm font-semibold text-white bg-indigo-600 rounded-lg
                  hover:bg-indigo-700 transition-colors"
              >
                {engine === "rev" ? "Next" : saving ? "Saving..." : "Finish Setup"}
              </button>
            </div>
          </div>
        )}

        {/* API key entry */}
        {step === "api-key" && (
          <div className="space-y-5">
            <div>
              <h2 className="text-lg font-semibold text-gray-900">
                Rev.AI API Key
              </h2>
              <p className="text-sm text-gray-500 mt-1">
                Head to{" "}
                <span className="text-indigo-600 font-medium">
                  rev.ai/auth/signup
                </span>{" "}
                to get an API key, then paste it below. We'll store it locally
                on your machine.
              </p>
            </div>

            <div className="space-y-3">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  API Key
                </label>
                <input
                  type="password"
                  value={revKey}
                  onChange={(e) => {
                    setRevKey(e.target.value);
                    setKeyError(null);
                  }}
                  placeholder="Paste your Rev.AI API key"
                  className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm
                    focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  Confirm API Key
                </label>
                <input
                  type="password"
                  value={revKeyConfirm}
                  onChange={(e) => {
                    setRevKeyConfirm(e.target.value);
                    setKeyError(null);
                  }}
                  placeholder="Paste again to confirm"
                  className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm
                    focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
                />
              </div>
            </div>

            {keyError && (
              <p className="text-sm text-red-600">{keyError}</p>
            )}

            <div className="flex justify-between pt-2">
              <button
                onClick={() => setStep("engine")}
                className="text-sm text-gray-500 hover:text-gray-700 transition-colors"
              >
                Back
              </button>
              <button
                onClick={handleSave}
                disabled={saving}
                className="px-6 py-2 text-sm font-semibold text-white bg-indigo-600 rounded-lg
                  hover:bg-indigo-700 transition-colors disabled:opacity-50"
              >
                {saving ? "Saving..." : "Save & Finish"}
              </button>
            </div>
          </div>
        )}

        {/* Done */}
        {step === "done" && (
          <div className="text-center space-y-6">
            <div>
              <h2 className="text-xl font-bold text-gray-900">
                You're all set!
              </h2>
              <p className="text-gray-600 mt-3">
                Default ASR engine:{" "}
                <span className="font-semibold">
                  {engine === "rev" ? "Rev.AI" : "Whisper"}
                </span>
              </p>
              <p className="text-sm text-gray-400 mt-2">
                You can change this anytime from Settings.
                Your configuration is saved at ~/.batchalign.ini
              </p>
            </div>
            <button
              onClick={onComplete}
              className="px-8 py-3 text-sm font-semibold text-white bg-emerald-600 rounded-lg
                hover:bg-emerald-700 transition-colors"
            >
              Get Started
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
