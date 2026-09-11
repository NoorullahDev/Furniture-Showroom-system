"use client";

import * as React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  CheckCircle2,
  DatabaseBackup,
  FolderOpen,
  Loader2,
  RefreshCw,
  RotateCcw,
  Trash2,
  Upload,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { useToast } from "@/components/ui/toast";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { isSessionError, useSession } from "@/components/session/session-provider";
import { commandErrorMessage } from "@/lib/tauri/client";
import {
  backupCreate,
  backupDelete,
  backupInspect,
  backupList,
  backupPreferencesGet,
  backupPreferencesSave,
  backupRestart,
  backupRestoreImport,
  maintenanceIntegrity,
  type BackupInspection,
  type BackupListItem,
  type BackupResult,
} from "@/lib/tauri/api";

function formatBytes(bytes: number) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index ? 1 : 0)} ${units[index]}`;
}

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value || "Unknown" : date.toLocaleString();
}

function generatedName() {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `Furniture-Shop-${stamp}.furniture-backup`;
}

export function MaintenancePage() {
  const { profile, refresh } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";
  const canBackup = profile?.permissions.includes("backup.create") ?? false;
  const canRestore = profile?.permissions.includes("backup.restore") ?? false;
  const [directory, setDirectory] = React.useState("");
  const [autoBackup, setAutoBackup] = React.useState(false);
  const [backupName, setBackupName] = React.useState(generatedName);
  const [lastBackup, setLastBackup] = React.useState<BackupResult | null>(null);
  const [restoreTarget, setRestoreTarget] = React.useState<BackupInspection | null>(null);
  const [deleteTarget, setDeleteTarget] = React.useState<BackupListItem | null>(null);

  const preferencesQuery = useQuery({
    queryKey: ["backup-preferences"],
    queryFn: () => backupPreferencesGet(session),
    enabled: !!session && canBackup,
  });
  const backupsQuery = useQuery({
    queryKey: ["backup-list", preferencesQuery.data?.directory],
    queryFn: () => backupList(session),
    enabled: !!session && canBackup && !!preferencesQuery.data?.directory,
    retry: false,
  });

  React.useEffect(() => {
    if (!preferencesQuery.data) return;
    setDirectory(preferencesQuery.data.directory ?? "");
    setAutoBackup(preferencesQuery.data.autoBackupOnClose);
  }, [preferencesQuery.data]);

  const saveMutation = useMutation({
    mutationFn: () => backupPreferencesSave(session, directory, autoBackup),
    onSuccess: (saved) => {
      queryClient.setQueryData(["backup-preferences"], saved);
      queryClient.invalidateQueries({ queryKey: ["backup-list"] });
      toast({ variant: "success", title: "Backup settings saved" });
    },
    onError: handleError("Could not save backup settings"),
  });
  const createMutation = useMutation({
    mutationFn: (name: string) => backupCreate(session, name),
    onSuccess: (result) => {
      setLastBackup(result);
      setBackupName(generatedName());
      queryClient.invalidateQueries({ queryKey: ["backup-list"] });
      toast({ variant: "success", title: "Backup created", description: result.backupPath });
    },
    onError: handleError("Backup failed"),
  });
  const inspectMutation = useMutation({
    mutationFn: (path: string) => backupInspect(session, path),
    onSuccess: setRestoreTarget,
    onError: handleError("Backup cannot be imported"),
  });
  const restoreMutation = useMutation({
    mutationFn: async (path: string) => {
      const result = await backupRestoreImport(session, path);
      await backupRestart(session);
      return result;
    },
    onError: handleError("Restore failed; current data was not changed"),
  });
  const deleteMutation = useMutation({
    mutationFn: (name: string) => backupDelete(session, name),
    onSuccess: () => {
      setDeleteTarget(null);
      queryClient.invalidateQueries({ queryKey: ["backup-list"] });
      toast({ variant: "success", title: "Backup deleted" });
    },
    onError: handleError("Could not delete backup"),
  });
  const integrityMutation = useMutation({
    mutationFn: () => maintenanceIntegrity(session),
    onSuccess: (result) => toast({
      variant: result.pageIntegrityOk && result.foreignKeyViolations === 0 ? "success" : "error",
      title: result.pageIntegrityOk && result.foreignKeyViolations === 0
        ? "Current database verified"
        : "Database integrity issues found",
    }),
    onError: handleError("Integrity check failed"),
  });

  function handleError(title: string) {
    return (error: unknown) => {
      if (isSessionError(error)) {
        refresh();
        return;
      }
      toast({ variant: "error", title, description: commandErrorMessage(error) });
    };
  }

  async function chooseFolder() {
    const selected = await open({ directory: true, multiple: false, title: "Select backup folder" });
    if (typeof selected === "string") setDirectory(selected);
  }

  async function chooseImport() {
    const selected = await open({
      directory: false,
      multiple: false,
      title: "Import a Furniture Showroom backup",
      filters: [
        { name: "Furniture Showroom backups", extensions: ["furniture-backup", "db"] },
      ],
    });
    if (typeof selected === "string") inspectMutation.mutate(selected);
  }

  if (!canBackup) {
    return <div className="rounded-lg border bg-white p-6 text-sm text-neutral-600">You do not have permission to manage backups.</div>;
  }

  const busy = saveMutation.isPending || createMutation.isPending || inspectMutation.isPending || restoreMutation.isPending;
  const settingsSaved = directory === (preferencesQuery.data?.directory ?? "")
    && autoBackup === (preferencesQuery.data?.autoBackupOnClose ?? false);

  function createBackup() {
    const name = /^Furniture-Shop-\d{8}-\d{6}\.furniture-backup$/i.test(backupName)
      ? generatedName()
      : backupName;
    setBackupName(name);
    createMutation.mutate(name);
  }

  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white shadow-sm">
      <div className="border-b border-neutral-200 px-5 py-4">
        <h2 className="text-sm font-semibold text-neutral-900">Backup &amp; Restore</h2>
        <p className="mt-1 text-xs text-neutral-500">Create complete local snapshots and safely restore shop data.</p>
      </div>

      <section className="space-y-4 border-b border-neutral-200 p-5">
        <div className="space-y-2">
          <Label htmlFor="backup-directory">Backup Directory</Label>
          <div className="flex gap-2">
            <Input id="backup-directory" value={directory} readOnly placeholder="Select a folder for manual and automatic backups" className="font-mono text-xs" />
            <Button type="button" variant="outline" onClick={chooseFolder} disabled={busy}>
              <FolderOpen className="h-4 w-4" /> Browse
            </Button>
          </div>
          <p className="text-xs text-neutral-500">Every backup is saved only in this folder. The full path is retained after restart.</p>
        </div>
        <label className="flex w-fit cursor-pointer items-center gap-3 text-sm text-neutral-800">
          <input
            type="checkbox"
            checked={autoBackup}
            onChange={(event) => setAutoBackup(event.target.checked)}
            className="h-4 w-4 rounded border-neutral-300 accent-forest-600"
          />
          <span>
            Auto-Backup on Close
            <span className="block text-xs font-normal text-neutral-500">Runs during a normal app exit; it cannot protect against power loss or forced termination.</span>
          </span>
        </label>
        <div className="flex justify-end">
          <Button onClick={() => saveMutation.mutate()} disabled={busy || !directory.trim()}>
            {saveMutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />} Save Settings
          </Button>
        </div>
      </section>

      <section className="space-y-4 border-b border-neutral-200 p-5">
        <div className="space-y-2">
          <Label htmlFor="backup-name">Backup Name</Label>
          <Input id="backup-name" value={backupName} onChange={(event) => setBackupName(event.target.value)} />
          <p className="text-xs text-neutral-500">A timestamp is added when needed. The package includes the SQLite snapshot, product images, logo, attachments, and local templates.</p>
        </div>
        <div className="flex flex-wrap justify-end gap-2">
          <Button onClick={createBackup} disabled={busy || !preferencesQuery.data?.directory || !settingsSaved || !backupName.trim()}>
            {createMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <DatabaseBackup className="h-4 w-4" />}
            {createMutation.isPending ? "Creating and validating…" : "Create Backup"}
          </Button>
          {canRestore && (
            <Button variant="outline" onClick={chooseImport} disabled={busy}>
              {inspectMutation.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Upload className="h-4 w-4" />}
              Import Backup
            </Button>
          )}
        </div>
        {lastBackup && (
          <div className="rounded-md border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-900">
            <div className="flex items-center gap-2 font-medium"><CheckCircle2 className="h-4 w-4" /> Backup saved and verified</div>
            <dl className="mt-2 grid gap-1 text-xs sm:grid-cols-[100px_1fr]">
              <dt>Filename</dt><dd className="font-mono break-all">{lastBackup.name}</dd>
              <dt>Full path</dt><dd className="font-mono break-all">{lastBackup.backupPath}</dd>
              <dt>Created</dt><dd>{formatDate(lastBackup.createdAt)}</dd>
              <dt>Size</dt><dd>{formatBytes(lastBackup.bytes)}</dd>
            </dl>
          </div>
        )}
      </section>

      <section className="p-5">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <h3 className="text-sm font-semibold text-neutral-900">Backups in selected folder</h3>
            <p className="max-w-3xl truncate font-mono text-xs text-neutral-500" title={preferencesQuery.data?.directory ?? ""}>{preferencesQuery.data?.directory || "No folder selected"}</p>
          </div>
          <div className="flex gap-1">
            <Button variant="ghost" size="sm" onClick={() => backupsQuery.refetch()} disabled={!preferencesQuery.data?.directory || backupsQuery.isFetching}>
              <RefreshCw className={`h-3.5 w-3.5 ${backupsQuery.isFetching ? "animate-spin" : ""}`} /> Refresh
            </Button>
            <Button variant="ghost" size="sm" onClick={() => integrityMutation.mutate()} disabled={integrityMutation.isPending}>
              Verify database
            </Button>
          </div>
        </div>

        {!preferencesQuery.data?.directory ? (
          <div className="rounded-md border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">Select a backup folder and save settings before creating a backup.</div>
        ) : backupsQuery.isLoading ? (
          <div className="flex items-center justify-center gap-2 py-10 text-sm text-neutral-500"><Loader2 className="h-4 w-4 animate-spin" /> Loading backups…</div>
        ) : backupsQuery.isError ? (
          <div className="rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-700"><AlertTriangle className="mr-2 inline h-4 w-4" />{commandErrorMessage(backupsQuery.error)}</div>
        ) : !backupsQuery.data?.length ? (
          <div className="py-10 text-center text-sm text-neutral-500">No backups found in the selected folder.</div>
        ) : (
          <div className="overflow-x-auto rounded-md border">
            <Table>
              <TableHeader><TableRow><TableHead>Backup</TableHead><TableHead>Created</TableHead><TableHead>Size</TableHead><TableHead>Status</TableHead><TableHead className="text-right">Actions</TableHead></TableRow></TableHeader>
              <TableBody>
                {backupsQuery.data.map((backup) => (
                  <TableRow key={backup.fullPath}>
                    <TableCell><div className="font-medium">{backup.name}</div><div className="max-w-md truncate font-mono text-[11px] text-neutral-500" title={backup.fullPath}>{backup.fullPath}</div></TableCell>
                    <TableCell>{formatDate(backup.createdAt)}</TableCell>
                    <TableCell>{formatBytes(backup.sizeBytes)}</TableCell>
                    <TableCell>{backup.verified ? <Badge variant="success">Verified</Badge> : <Badge variant="danger">Invalid</Badge>}</TableCell>
                    <TableCell><div className="flex justify-end gap-1">
                      {canRestore && <Button variant="ghost" size="sm" disabled={!backup.verified || busy} onClick={() => inspectMutation.mutate(backup.fullPath)}><RotateCcw className="h-3.5 w-3.5" /> Restore</Button>}
                      {canRestore && <Button variant="ghost" size="sm" className="text-red-600" disabled={busy} onClick={() => setDeleteTarget(backup)}><Trash2 className="h-3.5 w-3.5" /></Button>}
                    </div></TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </section>

      <Dialog open={!!restoreTarget} onOpenChange={(openState) => !openState && !restoreMutation.isPending && setRestoreTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Restore this backup?</DialogTitle>
            <DialogDescription>Restoring replaces the current shop data with this snapshot. A complete safety backup is created first, then the app restarts and requires login again.</DialogDescription>
          </DialogHeader>
          {restoreTarget && <dl className="grid grid-cols-[110px_1fr] gap-2 rounded-md bg-neutral-50 p-3 text-sm">
            <dt className="text-neutral-500">File</dt><dd className="break-all font-mono text-xs">{restoreTarget.name}</dd>
            <dt className="text-neutral-500">Created</dt><dd>{formatDate(restoreTarget.createdAt)}</dd>
            <dt className="text-neutral-500">Size</dt><dd>{formatBytes(restoreTarget.sizeBytes)}</dd>
            <dt className="text-neutral-500">Contents</dt><dd>{restoreTarget.legacyDatabaseOnly ? "Legacy database only" : `${restoreTarget.fileCount} verified files`}</dd>
            <dt className="text-neutral-500">Schema</dt><dd>{restoreTarget.schemaVersion}</dd>
          </dl>}
          <p className="text-xs text-neutral-500">The current backup destination is machine-specific and will be retained. Restoring does not activate a license.</p>
          <DialogFooter>
            <Button variant="outline" onClick={() => setRestoreTarget(null)} disabled={restoreMutation.isPending}>Cancel</Button>
            <Button variant="danger" onClick={() => restoreTarget && restoreMutation.mutate(restoreTarget.fullPath)} disabled={restoreMutation.isPending}>
              {restoreMutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />} Restore &amp; Restart
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={!!deleteTarget} onOpenChange={(openState) => !openState && setDeleteTarget(null)}>
        <DialogContent>
          <DialogHeader><DialogTitle>Delete backup?</DialogTitle><DialogDescription>This permanently removes {deleteTarget?.name} from the selected backup folder.</DialogDescription></DialogHeader>
          <DialogFooter><Button variant="outline" onClick={() => setDeleteTarget(null)}>Cancel</Button><Button variant="danger" disabled={deleteMutation.isPending} onClick={() => deleteTarget && deleteMutation.mutate(deleteTarget.name)}>{deleteMutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />} Delete</Button></DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
