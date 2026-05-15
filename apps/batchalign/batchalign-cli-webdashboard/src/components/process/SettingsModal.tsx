/** Settings modal for changing ASR engine and Rev.AI API key.
 *
 * Reads current config from `~/.batchalign.ini` via the desktop config
 * capability and writes changes back. Accessible from the gear icon in
 * the header. Only shown in desktop mode.
 */

import { useEffect, useRef, useState } from "react";
import {
  useDesktopConfig,
  useDesktopEnvironment,
} from "../../desktop/DesktopContext";

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
}

export function SettingsModal({ open, onClose }: SettingsModalProps) {
  const environment = useDesktopEnvironment();
  const config = useDesktopConfig();

  const [engine, setEngine] = useState("whisper");
  const [revKey, setRevKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load current config when modal opens
  useEffect(() => {
    if (!open || !environment.isDesktop) return;
    let cancelled = false;

    async function load() {
      try {
        const current = await config.readConfig();
        if (cancelled) return;
        setEngine(current.engine ?? "whisper");
        setRevKey(current.rev_key ?? "");
        setSaved(false);
        setError(null);
      } catch {
        // Defaults are fine
      }
    }

    void load();
    return () => { cancelled = true; };
  }, [open, environment, config]);

  const modalRef = useRef<HTMLDivElement>(null);

  // Close on Escape and trap focus inside modal
  useEffect(() => {
    if (!open) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      if (e.key === "Tab" && modalRef.current) {
        const focusable = modalRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    const closeBtn = modalRef.current?.querySelector<HTMLElement>("button");
    closeBtn?.focus();
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  async function handleSave() {
    if (engine === "rev" && !revKey.trim()) {
      setError("API key cannot be empty when using Rev.AI.");
      return;
    }

    setSaving(true);
    setError(null);
    try {
      await config.writeConfig({
        engine,
        rev_key: engine === "rev" ? revKey.trim() : null,
      });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      {/* Backdrop */}
      <div className="fixed inset-0 bg-black/30 z-40" onClick={onClose} aria-hidden="true" />

      {/* Modal */}
      <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div ref={modalRef} role="dialog" aria-modal="true" aria-label="Settings" className="bg-white rounded-xl shadow-xl max-w-md w-full p-6 space-y-5">
          {/* Header */}
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold text-gray-900">Settings</h2>
            <button
              onClick={onClose}
              className="text-gray-400 hover:text-gray-600 transition-colors"
              aria-label="Close settings"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          {/* ASR engine */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1.5">
              Default ASR engine
            </label>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setEngine("rev")}
                className={`flex-1 text-sm py-2 px-3 rounded-lg border-2 transition-colors ${
                  engine === "rev"
                    ? "border-indigo-500 bg-indigo-50 text-indigo-700 font-medium"
                    : "border-gray-200 text-gray-600 hover:border-gray-300"
                }`}
              >
                Rev.AI
                <div className="text-xs text-gray-400 mt-0.5 font-normal">Cloud, fast</div>
              </button>
              <button
                type="button"
                onClick={() => setEngine("whisper")}
                className={`flex-1 text-sm py-2 px-3 rounded-lg border-2 transition-colors ${
                  engine === "whisper"
                    ? "border-indigo-500 bg-indigo-50 text-indigo-700 font-medium"
                    : "border-gray-200 text-gray-600 hover:border-gray-300"
                }`}
              >
                Whisper
                <div className="text-xs text-gray-400 mt-0.5 font-normal">Local, free</div>
              </button>
            </div>
          </div>

          {/* Rev.AI key (shown only when rev selected) */}
          {engine === "rev" && (
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1.5">
                Rev.AI API key
              </label>
              <input
                type="password"
                value={revKey}
                onChange={(e) => {
                  setRevKey(e.target.value);
                  setError(null);
                  setSaved(false);
                }}
                placeholder="Paste your Rev.AI API key"
                className="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm
                  focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500"
              />
              <p className="text-xs text-gray-400 mt-1">
                Get a key at rev.ai/auth/signup
              </p>
            </div>
          )}

          {/* Error */}
          {error && (
            <p className="text-sm text-red-600">{error}</p>
          )}

          {/* Actions */}
          <div className="flex items-center justify-between pt-2">
            <span className="text-xs text-gray-400">
              Saved to ~/.batchalign.ini
            </span>
            <button
              type="button"
              onClick={handleSave}
              disabled={saving}
              className="px-5 py-2 text-sm font-semibold text-white bg-indigo-600 rounded-lg
                hover:bg-indigo-700 transition-colors disabled:opacity-50"
            >
              {saving ? "Saving..." : saved ? "Saved" : "Save"}
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
