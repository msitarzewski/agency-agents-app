/**
 * Activity store — tracks running + completed streaming jobs.
 * Drives the bottom Activity drawer + sidebar "Activity" section.
 *
 * Persistence: completed jobs (succeeded/failed/canceled) are mirrored to
 * localStorage so the Activity view survives app restarts. Running jobs are
 * NOT persisted — when the app dies the underlying backend operation dies
 * with it; there is no way to reattach to an in-flight install across launches.
 * On restore, any persisted "running" job is reclassified as "canceled" so the
 * historical record reflects reality.
 */

import type { ActivityJob, ActivityLine, AppStreamEvent } from "$lib/types";

const STORAGE_KEY = "agency-agents:activity:v1";
/** Cap how many jobs we persist. Older drop off the tail.
 *  Raised from 50 in earlier builds — most users never hit it, and
 *  a fatter history is more useful than the storage saving. */
const MAX_PERSISTED_JOBS = 200;
/** Cap log lines per job to keep localStorage write cost bounded. */
const MAX_LINES_PER_JOB = 500;
/** How long to wait after a state change before writing to localStorage. */
const PERSIST_DEBOUNCE_MS = 400;

interface PersistedShape {
  v: 1;
  jobs: ActivityJob[];
}

class ActivityStore {
  jobs: ActivityJob[] = $state([]);
  /** id of the job whose tab is selected in the drawer */
  activeJobId: string | null = $state(null);

  running = $derived(this.jobs.filter((j) => j.status === "running"));
  runningCount = $derived(this.running.length);

  private persistTimer: ReturnType<typeof setTimeout> | null = null;
  private hydrated = false;

  /**
   * Restore persisted jobs from localStorage. Safe to call multiple times — only
   * the first call hydrates. Should be invoked once during app bootstrap (e.g.
   * from `+layout.svelte`).
   */
  hydrate(): void {
    if (this.hydrated || typeof window === "undefined") return;
    this.hydrated = true;
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) {
        console.info("[activity] hydrate: no persisted entry (first launch or storage cleared)");
        return;
      }
      const parsed = JSON.parse(raw) as PersistedShape;
      if (!parsed || parsed.v !== 1 || !Array.isArray(parsed.jobs)) {
        console.warn("[activity] hydrate: persisted entry has unexpected shape; ignoring");
        return;
      }
      // Any "running" job in persisted state died with the previous process —
      // mark it canceled so the UI doesn't show a phantom spinner.
      const restored: ActivityJob[] = parsed.jobs.map((j) =>
        j.status === "running" ? { ...j, status: "canceled" } : j,
      );
      this.jobs = restored;
      this.activeJobId = restored[0]?.jobId ?? null;
      console.info(`[activity] hydrate: restored ${restored.length} job(s) from localStorage`);
    } catch (e) {
      console.warn(
        `[activity] hydrate failed (corrupt entry): ${
          e instanceof Error ? e.message : String(e)
        }`,
      );
      try { localStorage.removeItem(STORAGE_KEY); } catch { /* ignore */ }
    }
  }

  /**
   * Schedule a debounced write to localStorage. Coalesces rapid line bursts
   * into a single write at most every
   * PERSIST_DEBOUNCE_MS milliseconds.
   */
  private schedulePersist(): void {
    if (typeof window === "undefined") return;
    if (this.persistTimer) clearTimeout(this.persistTimer);
    this.persistTimer = setTimeout(() => {
      this.persistTimer = null;
      this.persistNow();
    }, PERSIST_DEBOUNCE_MS);
  }

  /**
   * Write current jobs to localStorage immediately. Truncates lines per job and
   * caps job count to keep storage bounded.
   *
   * On failure, logs a warning to the console (NOT a silent swallow) so a
   * persistence regression — quota exhaustion, serialization failure, a
   * webview-side storage policy quirk — is visible during dev/testing
   * instead of presenting as "the activity log silently empties."
   */
  private persistNow(): void {
    if (typeof window === "undefined") return;
    try {
      const trimmed: ActivityJob[] = this.jobs
        .slice(0, MAX_PERSISTED_JOBS)
        .map((j) => {
          if (j.lines.length <= MAX_LINES_PER_JOB) return j;
          // Keep the last N lines — install errors usually surface at the end.
          const lines: ActivityLine[] = j.lines.slice(-MAX_LINES_PER_JOB);
          return { ...j, lines };
        });
      const payload: PersistedShape = { v: 1, jobs: trimmed };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
    } catch (e) {
      console.warn(
        `[activity] persistNow failed (jobs=${this.jobs.length}): ${
          e instanceof Error ? e.message : String(e)
        }`,
      );
    }
  }

  startJob(label: string, jobId: string, command: string) {
    const job: ActivityJob = {
      jobId,
      label,
      command,
      startedAt: new Date().toISOString(),
      status: "running",
      lines: [],
    };
    this.jobs = [job, ...this.jobs];
    this.activeJobId = jobId;
    // Persist IMMEDIATELY so even a hard kill within the 400ms
    // debounce window leaves the job in the history. Streaming
    // line bursts still ride the debounced path below.
    this.persistNow();
  }

  handleEvent(evt: AppStreamEvent) {
    const idx = this.jobs.findIndex((j) => j.jobId === evt.jobId);
    if (idx === -1) {
      // event for an unknown job — could happen on race conditions; ignore quietly.
      return;
    }
    const j = this.jobs[idx];
    switch (evt.kind) {
      case "started":
        // already recorded at startJob; refresh command if useful
        j.command = evt.command;
        break;
      case "stdout":
        j.lines = [...j.lines, { stream: "stdout", text: evt.line, ts: evt.ts }];
        break;
      case "stderr":
        j.lines = [...j.lines, { stream: "stderr", text: evt.line, ts: evt.ts }];
        break;
      case "progress":
        // soft-record progress as a line for now (UI can read percent later)
        j.lines = [...j.lines, { stream: "stdout", text: `[progress] ${evt.message}`, ts: new Date().toISOString() }];
        break;
      case "exit":
        j.status = evt.success ? "succeeded" : "failed";
        j.exitCode = evt.exitCode;
        j.durationMs = evt.durationMs;
        break;
      case "canceled":
        j.status = "canceled";
        break;
      case "error":
        j.status = "failed";
        j.lines = [...j.lines, { stream: "stderr", text: `[error] ${evt.error.code}`, ts: new Date().toISOString() }];
        break;
    }
    // re-publish (Svelte 5 deep-mutation works, but reassign to be explicit)
    this.jobs = [...this.jobs];
    // Terminal events flush immediately; mid-stream updates are debounced.
    if (evt.kind === "exit" || evt.kind === "canceled" || evt.kind === "error") {
      this.persistNow();
    } else {
      this.schedulePersist();
    }
  }

  setActive(jobId: string) { this.activeJobId = jobId; }

  removeJob(jobId: string) {
    this.jobs = this.jobs.filter((j) => j.jobId !== jobId);
    if (this.activeJobId === jobId) {
      this.activeJobId = this.jobs[0]?.jobId ?? null;
    }
    this.persistNow();
  }

  clearCompleted() {
    this.jobs = this.jobs.filter((j) => j.status === "running");
    this.persistNow();
  }

  /** Wipe all history including the localStorage mirror. */
  clearAll() {
    this.jobs = [];
    this.activeJobId = null;
    this.persistNow();
  }

  /** Mark a running job canceled locally. The brew-era backend
   *  `cancel_job` IPC was retired with the brew domain; agency install
   *  jobs are not yet backend-cancellable, so this updates the local
   *  record only. Re-wire to a backend cancel when streaming agency
   *  jobs land. */
  cancel(jobId: string) {
    const idx = this.jobs.findIndex((j) => j.jobId === jobId);
    if (idx === -1) return;
    if (this.jobs[idx].status === "running") {
      this.jobs[idx].status = "canceled";
      this.jobs = [...this.jobs];
      this.persistNow();
    }
  }
}

export const activity = new ActivityStore();
