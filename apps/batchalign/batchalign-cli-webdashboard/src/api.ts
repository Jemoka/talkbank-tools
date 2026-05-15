/** REST fetch helpers for the Batchalign server API. */

import type { JobInfo, JobListItem } from "./types";
import {
  controlPlaneHostPort,
  controlPlaneHttpScheme,
  controlPlaneOrigin,
  isSameControlPlaneAsPage,
} from "./runtime";

export class ApiError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

async function parseJson<T>(res: Response, context: string): Promise<T> {
  if (!res.ok) {
    throw new ApiError(`${context} failed: ${res.status}`, res.status);
  }
  return (await res.json()) as T;
}

/** Build the HTTP base URL for a server host:port string. */
export function serverBaseUrl(hostPort: string): string {
  if (isSameControlPlaneAsPage(hostPort)) return "";
  if (hostPort === controlPlaneHostPort()) return controlPlaneOrigin();
  return `${controlPlaneHttpScheme()}//${hostPort}`;
}

export async function fetchJobs(base = ""): Promise<JobListItem[]> {
  const res = await fetch(`${base}/jobs`);
  return parseJson<JobListItem[]>(res, "GET /jobs");
}

export async function fetchJob(jobId: string, base = ""): Promise<JobInfo> {
  const res = await fetch(`${base}/jobs/${jobId}`);
  return parseJson<JobInfo>(res, `GET /jobs/${jobId}`);
}

export async function cancelJob(
  jobId: string,
  base = ""
): Promise<{ status: string; message: string }> {
  const res = await fetch(`${base}/jobs/${jobId}/cancel`, { method: "POST" });
  return parseJson<{ status: string; message: string }>(res, "POST cancel");
}

export async function restartJob(jobId: string, base = ""): Promise<JobInfo> {
  const res = await fetch(`${base}/jobs/${jobId}/restart`, { method: "POST" });
  return parseJson<JobInfo>(res, "POST restart");
}

export async function deleteJob(
  jobId: string,
  base = ""
): Promise<{ status: string; message: string }> {
  const res = await fetch(`${base}/jobs/${jobId}`, { method: "DELETE" });
  return parseJson<{ status: string; message: string }>(res, "DELETE job");
}

/** Submit a new processing job to the server. */
export async function submitJob(
  body: {
    command: string;
    lang?: string;
    num_speakers?: number;
    paths_mode: boolean;
    source_paths: string[];
    output_paths: string[];
    source_dir?: string;
    options: Record<string, unknown>;
  },
  base = "",
): Promise<import("./types").JobInfo> {
  const res = await fetch(`${base}/jobs`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  return parseJson<import("./types").JobInfo>(res, "POST /jobs");
}

/** Check server health. */
export async function fetchHealth(
  base = "",
): Promise<import("./types").HealthResponse> {
  const res = await fetch(`${base}/health`);
  return parseJson<import("./types").HealthResponse>(res, "GET /health");
}
