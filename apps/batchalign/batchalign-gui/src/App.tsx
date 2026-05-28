// The 5-state shell. Decides which view to render from one read of the
// store. Boots the bridge on mount.

import { useEffect } from "react";
import { useStore } from "./store";
import { bootBridge } from "./bridge";
import BAWindow from "./components/BAWindow";
import BAHeader from "./components/BAHeader";
import TabBar from "./components/TabBar";
import EmptyView from "./views/EmptyView";
import BatchView from "./views/BatchView";
import SettingsView from "./views/SettingsView";

export default function App() {
  const { tabOrder, daemon, showSettings, dispatch } = useStore();

  useEffect(() => {
    let cleanup: (() => void) | null = null;
    let cancelled = false;
    bootBridge().then((fn) => {
      if (cancelled) fn();
      else cleanup = fn;
    });
    return () => {
      cancelled = true;
      if (cleanup) cleanup();
    };
  }, []);

  const isEmpty = tabOrder.length === 0 && !showSettings;

  return (
    <BAWindow>
      {!showSettings && <TabBar />}
      <BAHeader
        sub={daemon.ready ? null : daemon.error ? "daemon error" : "starting daemon…"}
        right={
          <div style={{ display: "flex", gap: 8 }}>
            <button
              type="button"
              className={`ba-btn ba-btn--sm${showSettings ? " ba-btn--primary" : ""}`}
              onClick={() =>
                dispatch({
                  type: "SETTINGS_TOGGLED",
                  show: !showSettings,
                })
              }
            >
              {showSettings ? "close" : "settings"}
            </button>
          </div>
        }
      />
      {showSettings ? (
        <SettingsView />
      ) : isEmpty ? (
        <EmptyView />
      ) : (
        <BatchView />
      )}
    </BAWindow>
  );
}
