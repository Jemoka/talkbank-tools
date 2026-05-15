/** Header + main content wrapper.
 *
 * Shows the app branding, navigation links (Process and Dashboard),
 * compact job stats, connection status indicators, and a help button
 * that opens a slide-out panel with command descriptions and FAQ.
 */

import { useState, type ReactNode } from "react";
import { useLocation } from "wouter";
import { useAnyConnected, useServerStatuses, useStats } from "../state";
import { useDesktopEnvironment } from "../desktop/DesktopContext";
import { HelpPanel } from "./process/HelpPanel";
import { SettingsModal } from "./process/SettingsModal";

export function Layout({ children }: { children: ReactNode }) {
  const environment = useDesktopEnvironment();
  const anyConnected = useAnyConnected();
  const statuses = useServerStatuses();
  const multiServer = statuses.length > 1;
  const stats = useStats();
  const [location] = useLocation();
  const [helpOpen, setHelpOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  /** Active link style: brighter text for the current route. */
  function navClass(href: string): string {
    const isActive =
      href === "/" ? location === href : location.startsWith(href);
    return `text-xs no-underline ${
      isActive
        ? "text-white/90 font-medium"
        : "text-white/50 hover:text-white/80"
    }`;
  }

  return (
    <div className="min-h-screen">
      <a className="skip-to-content" href="#main">Skip to main content</a>
      <header className="bg-[var(--bg-header)] text-white">
        <div className="max-w-5xl mx-auto px-4 py-3 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <a
              href={environment.isDesktop ? "/process" : "/dashboard"}
              className="flex items-center gap-2 no-underline"
            >
              <span className="font-mono text-sm font-semibold tracking-tight text-white/90">
                batchalign
              </span>
              {!environment.isDesktop && (
                <span className="text-white/30 text-xs font-light">dashboard</span>
              )}
            </a>

            {/* Navigation links */}
            <nav className="flex items-center gap-3" aria-label="Main navigation">
              {environment.isDesktop && (
                <a href="/process" className={navClass("/process")}>
                  Process
                </a>
              )}
              <a href="/dashboard" className={navClass("/dashboard")}>
                Dashboard
              </a>
              <a
                href="/dashboard/visualizations"
                className={`${navClass("/dashboard/visualizations")} hidden sm:block`}
              >
                Visualizations
              </a>
            </nav>
          </div>

          <div className="flex items-center gap-4 text-sm">
            {/* Compact stats in header */}
            <div className="hidden sm:flex items-center gap-3 text-xs text-white/50">
              {stats.active > 0 && (
                <span>
                  <span className="text-blue-400 font-medium">{stats.active}</span> active
                </span>
              )}
              {stats.completed > 0 && (
                <span>
                  <span className="text-emerald-400 font-medium">{stats.completed}</span> done
                </span>
              )}
              {stats.failed > 0 && (
                <span>
                  <span className="text-red-400 font-medium">{stats.failed}</span> failed
                </span>
              )}
            </div>

            {/* Connection status */}
            <div className="flex items-center gap-2">
              {multiServer ? (
                statuses.map(({ server, connected }) => (
                  <div key={server} className="flex items-center gap-1.5">
                    <span
                      className={`inline-block w-1.5 h-1.5 rounded-full ${
                        connected ? "bg-emerald-400" : "bg-red-400"
                      }`}
                    />
                    <span className="text-xs text-white/50">{server}</span>
                  </div>
                ))
              ) : (
                <div className="flex items-center gap-1.5">
                  <span
                    className={`inline-block w-1.5 h-1.5 rounded-full ${
                      anyConnected ? "bg-emerald-400" : "bg-red-400"
                    }`}
                  />
                  <span className="text-xs text-white/50">
                    {anyConnected ? "Connected" : "Reconnecting\u2026"}
                  </span>
                </div>
              )}
            </div>

            {/* Settings button (desktop only) */}
            {environment.isDesktop && (
              <button
                onClick={() => setSettingsOpen(true)}
                className="text-white/40 hover:text-white/80 transition-colors"
                aria-label="Settings"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                    d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
              </button>
            )}

            {/* Help button */}
            <button
              onClick={() => setHelpOpen(true)}
              className="text-white/40 hover:text-white/80 transition-colors"
              aria-label="Help"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                  d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </button>
          </div>
        </div>
      </header>
      <main id="main" className="max-w-5xl mx-auto px-4 py-6">{children}</main>
      <HelpPanel open={helpOpen} onClose={() => setHelpOpen(false)} />
      <SettingsModal open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
