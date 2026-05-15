/** Focused React contexts for the dashboard desktop runtime seam.
 *
 * The low-level Tauri adapter still owns IPC transport in `lib/tauri.ts`, but
 * the React tree now consumes separate environment, file, config, and server
 * capabilities so each consumer depends on the smallest possible surface.
 */

import {
  type Context,
  createContext,
  useContext,
  useMemo,
  type ReactNode,
} from "react";
import { isDesktop } from "../runtime";
import {
  discoverFiles,
  isFirstLaunch,
  onServerStatusChange,
  openPath,
  pickFolder,
  pickOutputFolder,
  readConfig,
  serverStatus,
  startServer,
  stopServer,
  writeConfig,
} from "../lib/tauri";
import type {
  DesktopConfigCapability,
  DesktopEnvironmentCapability,
  DesktopFilesCapability,
  DesktopServerCapability,
} from "./capabilities";

const DesktopEnvironmentContext =
  createContext<DesktopEnvironmentCapability | null>(null);
const DesktopFilesContext = createContext<DesktopFilesCapability | null>(null);
const DesktopConfigContext = createContext<DesktopConfigCapability | null>(null);
const DesktopServerContext = createContext<DesktopServerCapability | null>(null);

/** Provide the shared desktop runtime seam to the entire React tree. */
export function DesktopProvider({ children }: { children: ReactNode }) {
  const environment = useMemo<DesktopEnvironmentCapability>(
    () => ({ isDesktop: isDesktop() }),
    [],
  );
  const files = useMemo<DesktopFilesCapability>(
    () => ({
      pickFolder,
      pickOutputFolder,
      discoverFiles,
      openPath,
    }),
    [],
  );
  const config = useMemo<DesktopConfigCapability>(
    () => ({
      isFirstLaunch,
      readConfig,
      writeConfig,
    }),
    [],
  );
  const server = useMemo<DesktopServerCapability>(
    () => ({
      serverStatus,
      startServer,
      stopServer,
      onServerStatusChange,
    }),
    [],
  );

  return (
    <DesktopEnvironmentContext.Provider value={environment}>
      <DesktopFilesContext.Provider value={files}>
        <DesktopConfigContext.Provider value={config}>
          <DesktopServerContext.Provider value={server}>
            {children}
          </DesktopServerContext.Provider>
        </DesktopConfigContext.Provider>
      </DesktopFilesContext.Provider>
    </DesktopEnvironmentContext.Provider>
  );
}

function useDesktopCapability<T>(
  context: Context<T | null>,
  hookName: string,
): T {
  const value = useContext(context);
  if (!value) {
    throw new Error(`${hookName} must be used within a DesktopProvider`);
  }
  return value;
}

/** Read the runtime-only desktop capability. */
export function useDesktopEnvironment(): DesktopEnvironmentCapability {
  return useDesktopCapability(DesktopEnvironmentContext, "useDesktopEnvironment");
}

/** Read the file-system desktop capability. */
export function useDesktopFiles(): DesktopFilesCapability {
  return useDesktopCapability(DesktopFilesContext, "useDesktopFiles");
}

/** Read the config desktop capability. */
export function useDesktopConfig(): DesktopConfigCapability {
  return useDesktopCapability(DesktopConfigContext, "useDesktopConfig");
}

/** Read the server-lifecycle desktop capability. */
export function useDesktopServer(): DesktopServerCapability {
  return useDesktopCapability(DesktopServerContext, "useDesktopServer");
}
