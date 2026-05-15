const DEFAULT_DESKTOP_SERVER = "http://127.0.0.1:18000";

/** True when running inside a Tauri webview (desktop app). */
export function isDesktop(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

function normalizeServerUrl(raw: string): URL | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;

  try {
    if (trimmed.includes("://")) return new URL(trimmed);
    return new URL(`http://${trimmed}`);
  } catch {
    return null;
  }
}

function serverOverrideFromQuery(): URL | null {
  const params = new URLSearchParams(window.location.search);
  return (
    normalizeServerUrl(params.get("server") ?? "") ??
    normalizeServerUrl(params.get("api") ?? "")
  );
}

export function controlPlaneUrl(): URL {
  const override = serverOverrideFromQuery();
  if (override) return override;

  if (location.protocol === "http:" || location.protocol === "https:") {
    return new URL(`${location.protocol}//${location.host}`);
  }

  return new URL(DEFAULT_DESKTOP_SERVER);
}

export function controlPlaneOrigin(): string {
  return controlPlaneUrl().origin;
}

export function controlPlaneHostPort(): string {
  return controlPlaneUrl().host;
}

export function controlPlaneHttpScheme(): "http:" | "https:" {
  return controlPlaneUrl().protocol === "https:" ? "https:" : "http:";
}

export function controlPlaneWsScheme(): "ws:" | "wss:" {
  return controlPlaneUrl().protocol === "https:" ? "wss:" : "ws:";
}

export function isSameControlPlaneAsPage(hostPort: string): boolean {
  const control = controlPlaneUrl();
  if (hostPort !== control.host) return false;

  if (location.protocol !== "http:" && location.protocol !== "https:") {
    return false;
  }

  return control.protocol === location.protocol && control.host === location.host;
}
