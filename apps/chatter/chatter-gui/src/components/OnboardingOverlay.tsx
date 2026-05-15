import { useCallback, useState } from "react";

const STORAGE_KEY = "chatter-onboarding-dismissed";

export default function OnboardingOverlay() {
  const [dismissed, setDismissed] = useState(
    () => localStorage.getItem(STORAGE_KEY) === "1",
  );

  const handleDismiss = useCallback(() => {
    localStorage.setItem(STORAGE_KEY, "1");
    setDismissed(true);
  }, []);

  if (dismissed) return null;

  const isMac =
    typeof navigator !== "undefined" && /Mac/.test(navigator.userAgent);
  const mod = isMac ? "\u2318" : "Ctrl+";

  return (
    <div className="onboarding-backdrop" onClick={handleDismiss}>
      <div className="onboarding-card" onClick={(e) => e.stopPropagation()}>
        <h2>Welcome to Chatter</h2>
        <div className="onboarding-callouts">
          <div className="onboarding-callout">
            <span className="onboarding-icon">{"\uD83D\uDCC2"}</span>
            <p>
              <strong>Drag a .cha file or folder</strong> onto the window, or
              use the buttons to pick one.
            </p>
          </div>
          <div className="onboarding-callout">
            <span className="onboarding-icon">{"\uD83D\uDD0D"}</span>
            <p>
              <strong>Errors appear on the right</strong> with full source
              context. Click a file in the tree to see its diagnostics.
            </p>
          </div>
          <div className="onboarding-callout">
            <span className="onboarding-icon">{"\u2328\uFE0F"}</span>
            <p>
              <strong>{mod}R</strong> re-validates,{" "}
              <strong>Esc</strong> cancels a running validation.
            </p>
          </div>
          <div className="onboarding-callout">
            <span className="onboarding-icon">{"\uD83D\uDCBE"}</span>
            <p>
              <strong>Export results</strong> as JSON or plain text from the
              status bar when validation finishes.
            </p>
          </div>
        </div>
        <button className="primary dismiss-btn" onClick={handleDismiss}>
          Got it
        </button>
      </div>
    </div>
  );
}
