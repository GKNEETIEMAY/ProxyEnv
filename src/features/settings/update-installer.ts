import type { DownloadEvent } from "@tauri-apps/plugin-updater";

export interface InstallHandle {
  download(callback: (event: DownloadEvent) => void, options: { timeout: number }): Promise<void>;
  install(): Promise<void>;
  close(): Promise<void>;
}
export interface InstallGuard {
  prepare(confirmed: boolean): Promise<number>;
  validate(lease: number): Promise<void>;
  release(lease: number): Promise<void>;
}
export class UpdateInstaller {
  busy = false;
  abandoned = false;
  phase: "prepare" | "download" | "install" = "prepare";
  private lease: number | null = null;
  get guardReleased(): boolean { return this.lease === null; }
  constructor(private readonly guard: InstallGuard) {}

  async cancel(): Promise<void> {
    if (!this.busy || this.phase !== "download") return;
    // Tauri's download API has no AbortSignal. Abandon installation, release the
    // mutation gate immediately, and let the bounded download close its resource.
    this.abandoned = true;
    await this.release();
  }
  private async release(): Promise<void> {
    const lease = this.lease;
    if (lease === null) return;
    await this.guard.release(lease);
    this.lease = null;
  }
  async run(handle: InstallHandle, confirm: () => Promise<boolean>, onPhase: (phase: "download" | "install") => void,
    onProgress: (event: DownloadEvent) => void): Promise<"deferred" | "cancelled" | "installed"> {
    if (this.busy) throw new Error("updateBusy");
    this.busy = true;
    this.abandoned = false;
    this.phase = "prepare";
    let consumed = false;
    try {
      try { this.lease = await this.guard.prepare(false); }
      catch (error) {
        if (error !== "updateConfirmationRequired") throw error;
        if (!await confirm()) return "deferred";
        this.lease = await this.guard.prepare(true);
      }
      this.phase = "download";
      consumed = true;
      onPhase("download");
      await handle.download(event => { if (!this.abandoned) onProgress(event); }, { timeout: 90_000 });
      // download() resolves only AFTER Tauri has verified the artifact signature.
      if (this.abandoned) return "cancelled";
      const lease = this.lease;
      if (lease === null) throw new Error("updateStateChanged");
      await this.guard.validate(lease);
      if (this.abandoned) return "cancelled";
      this.phase = "install";
      onPhase("install");
      // No post-install relaunch guard: Windows can exit inside install().
      await handle.install();
      return "installed";
    } finally {
      try { await this.release(); }
      finally { this.busy = false; if (consumed) await handle.close().catch(() => {}); }
    }
  }
}
