"use client";

import * as React from "react";
import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { backupCloseCancel, backupCloseRetry, backupCloseWithout } from "@/lib/tauri/api";
import { useSession } from "@/components/session/session-provider";

type CloseState = { mode: "idle" | "running" | "failed"; message?: string };

export function BackupCloseGuard() {
  const [state, setState] = React.useState<CloseState>({ mode: "idle" });
  const { profile } = useSession();
  const session = profile?.sessionId ?? "";

  React.useEffect(() => {
    const cleanups = Promise.all([
      listen("backup-close-progress", () => setState({ mode: "running" })),
      listen<{ message?: string }>("backup-close-failed", (event) => setState({
        mode: "failed",
        message: event.payload?.message || "The automatic backup could not be completed.",
      })),
      listen("backup-close-complete", () => setState({ mode: "running" })),
    ]).catch(() => []);
    return () => { void cleanups.then((listeners) => listeners.forEach((unlisten) => unlisten())); };
  }, []);

  async function cancelClose() {
    await backupCloseCancel(session);
    setState({ mode: "idle" });
  }

  async function retry() {
    setState({ mode: "running" });
    try {
      await backupCloseRetry(session);
    } catch {
      // The backend emits the precise failure and returns to the choice dialog.
    }
  }

  return (
    <Dialog open={state.mode !== "idle"}>
      <DialogContent
        className="max-w-md"
        onEscapeKeyDown={(event) => event.preventDefault()}
        onPointerDownOutside={(event) => event.preventDefault()}
      >
        {state.mode === "running" ? (
          <>
            <DialogHeader>
              <DialogTitle>Creating backup before closing</DialogTitle>
              <DialogDescription>The app is waiting for pending writes, saving the complete package, and validating it.</DialogDescription>
            </DialogHeader>
            <div className="flex items-center gap-3 rounded-md bg-neutral-50 p-4 text-sm text-neutral-700">
              <Loader2 className="h-5 w-5 animate-spin text-forest-600" /> Please keep the application open…
            </div>
          </>
        ) : (
          <>
            <DialogHeader>
              <DialogTitle>Automatic backup failed</DialogTitle>
              <DialogDescription>The application is still open and your current data remains usable.</DialogDescription>
            </DialogHeader>
            <div className="flex gap-3 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-800">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
              <span>{state.message}</span>
            </div>
            <DialogFooter className="sm:justify-between">
              <Button variant="outline" onClick={cancelClose}>Cancel Close</Button>
              <div className="flex gap-2">
                <Button variant="danger" onClick={() => void backupCloseWithout(session)}>Close Without Backup</Button>
                <Button onClick={retry}>Retry</Button>
              </div>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
