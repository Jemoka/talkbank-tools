/** Dashboard application composition root.
 *
 * This file intentionally stays tiny. Bootstrapping fleet discovery,
 * query synchronization, and WebSocket live updates happens inside
 * `useFleetDashboardSync()`, while route composition lives in `AppRoutes`.
 *
 * In desktop mode, checks for first-launch (no `~/.batchalign.ini`) and
 * shows the setup wizard before the main route tree. This matches the
 * batchalign2 behavior where `config_read(interactive=True)` triggers
 * `interactive_setup()` on first CLI invocation.
 */
import { useCallback, useEffect, useState } from "react";
import { AppRoutes } from "./AppRoutes";
import { useFleetDashboardSync } from "./hooks/useFleetDashboardSync";
import { SetupWizard } from "./components/setup/SetupWizard";
import {
  useDesktopConfig,
  useDesktopEnvironment,
} from "./desktop/DesktopContext";

/** Mount the dashboard route tree after starting the fleet-sync side effects. */
export function App() {
  useFleetDashboardSync();
  const environment = useDesktopEnvironment();
  const desktopConfig = useDesktopConfig();

  const [showWizard, setShowWizard] = useState<boolean | null>(null);

  const handleSetupComplete = useCallback(() => {
    setShowWizard(false);
    if (environment.isDesktop && window.location.pathname === "/") {
      window.history.replaceState(
        null,
        "",
        `/process${window.location.search}`,
      );
      window.dispatchEvent(new PopStateEvent("popstate"));
    }
  }, [environment]);

  useEffect(() => {
    let cancelled = false;

    async function loadFirstLaunchState() {
      const next = environment.isDesktop
        ? await desktopConfig.isFirstLaunch()
        : false;
      if (!cancelled) {
        setShowWizard(next);
      }
    }

    void loadFirstLaunchState();
    return () => {
      cancelled = true;
    };
  }, [desktopConfig, environment]);

  // Still checking — render nothing briefly
  if (showWizard === null && environment.isDesktop) return null;

  if (showWizard) {
    return <SetupWizard onComplete={handleSetupComplete} />;
  }

  return <AppRoutes />;
}
