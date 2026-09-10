"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, HardDrive, Shield, Trash2, Undo2, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/components/page-header";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  maintenanceStatus,
  backupCreate,
  backupList,
  backupDelete,
  backupRestore,
  maintenanceIntegrity,
  type BackupListItem,
  type IntegrityResult,
} from "@/lib/tauri/api";
import { isSessionError, useSession } from "@/components/session/session-provider";
import { commandErrorMessage } from "@/lib/tauri/client";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function formatDateTime(iso: string | null): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleString();
}

function StatusCard({
  label,
  value,
  icon: Icon,
  accent,
}: {
  label: string;
  value: string;
  icon: React.ElementType;
  accent?: boolean;
}) {
  return (
    <div className="rounded-lg border bg-white p-4 shadow-sm">
      <div className="flex items-center gap-2 text-sm text-neutral-500">
        <Icon className="h-4 w-4" />
        {label}
      </div>
      <p className={`mt-1 truncate text-lg font-semibold ${accent ? "text-emerald-700" : "text-neutral-900"}`}>
        {value}
      </p>
    </div>
  );
}

export function MaintenancePage() {
  const { toast } = useToast();
  const { profile, refresh } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";
  const canBackup = profile?.permissions.includes("backup.create") ?? false;
  const canRestore = profile?.permissions.includes("backup.restore") ?? false;

  const [deleteTarget, setDeleteTarget] = React.useState<BackupListItem | null>(null);
  const [restoreTarget, setRestoreTarget] = React.useState<BackupListItem | null>(null);
  const [restoreConfirm, setRestoreConfirm] = React.useState("");

  // --- Queries ---
  const statusQuery = useQuery({
    queryKey: ["maintenance-status"],
    queryFn: () => maintenanceStatus(session),
    enabled: !!session && canBackup,
  });

  const backupQuery = useQuery({
    queryKey: ["backup-list"],
    queryFn: () => backupList(session),
    enabled: !!session && canBackup,
  });

  // --- Mutations ---
  const createMutation = useMutation({
    mutationFn: () => backupCreate(session),
    onSuccess: () => {
      toast({ variant: "success", title: "Backup created" });
      queryClient.invalidateQueries({ queryKey: ["backup-list"] });
      queryClient.invalidateQueries({ queryKey: ["maintenance-status"] });
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) { refresh(); return; }
      toast({ variant: "error", title: "Backup failed", description: commandErrorMessage(e) });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (name: string) => backupDelete(session, name),
    onSuccess: () => {
      toast({ variant: "success", title: "Backup deleted" });
      setDeleteTarget(null);
      queryClient.invalidateQueries({ queryKey: ["backup-list"] });
      queryClient.invalidateQueries({ queryKey: ["maintenance-status"] });
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) { refresh(); return; }
      toast({ variant: "error", title: "Delete failed", description: commandErrorMessage(e) });
    },
  });

  const restoreMutation = useMutation({
    mutationFn: (name: string) => backupRestore(session, name),
    onSuccess: (result) => {
      toast({
        variant: "success",
        title: "Restore scheduled",
        description: result.restartRequired
          ? `Restart required. Safety backup: ${result.safetyBackupName ?? "created"}`
          : undefined,
      });
      setRestoreTarget(null);
      setRestoreConfirm("");
      queryClient.invalidateQueries({ queryKey: ["backup-list"] });
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) { refresh(); return; }
      toast({ variant: "error", title: "Restore failed", description: commandErrorMessage(e) });
    },
  });

  const integrityMutation = useMutation({
    mutationFn: () => maintenanceIntegrity(session),
    onSuccess: (result: IntegrityResult) => {
      const msg = result.pageIntegrityOk && result.foreignKeyViolations === 0
        ? "Database integrity verified"
        : `Integrity issues found (FK violations: ${result.foreignKeyViolations})`;
      toast({ variant: result.pageIntegrityOk && result.foreignKeyViolations === 0 ? "success" : "error", title: msg });
      queryClient.invalidateQueries({ queryKey: ["maintenance-status"] });
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) { refresh(); return; }
      toast({ variant: "error", title: "Integrity check failed", description: commandErrorMessage(e) });
    },
  });

  if (!canBackup) {
    return (
      <div className="space-y-6">
        <PageHeader title="Backup & maintenance" subtitle="You do not have permission to view this page." />
      </div>
    );
  }

  const s = statusQuery.data;

  return (
    <div className="space-y-6">
      <PageHeader
        title="Backup & maintenance"
        subtitle="Database status, backup management and integrity checks."
      />

      {/* Status cards */}
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <StatusCard
          label="Database"
          value={s ? formatBytes(s.dbSizeBytes) : "—"}
          icon={HardDrive}
        />
        <StatusCard
          label="Images"
          value={s ? formatBytes(s.imagesSizeBytes) : "—"}
          icon={HardDrive}
        />
        <StatusCard
          label="Backups"
          value={s ? formatBytes(s.backupsSizeBytes) : "—"}
          icon={HardDrive}
        />
        <StatusCard
          label="Free disk"
          value={s?.freeDiskBytes != null ? formatBytes(s.freeDiskBytes) : "—"}
          icon={HardDrive}
        />
      </div>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <StatusCard
          label="App version"
          value={s?.appVersion ?? "—"}
          icon={HardDrive}
        />
        <StatusCard
          label="Schema version"
          value={s != null ? `${s.schemaVersion} (${s.pendingMigrations} pending)` : "—"}
          icon={HardDrive}
        />
        <StatusCard
          label="Last backup"
          value={s?.lastBackupName ?? "None"}
          icon={HardDrive}
          accent={!!s?.lastBackupName}
        />
        <StatusCard
          label="Last integrity"
          value={
            s?.lastIntegrityAt
              ? `${s.lastIntegrityOk ? "OK" : "FAILED"} · ${formatDateTime(s.lastIntegrityAt)}`
              : "Never checked"
          }
          icon={Shield}
          accent={s?.lastIntegrityOk === true}
        />
      </div>

      {/* Actions */}
      <div className="flex flex-wrap gap-3">
        <Button
          onClick={() => createMutation.mutate()}
          disabled={createMutation.isPending}
        >
          {createMutation.isPending ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <HardDrive className="mr-2 h-4 w-4" />}
          Back up now
        </Button>
        <Button
          variant="outline"
          onClick={() => integrityMutation.mutate()}
          disabled={integrityMutation.isPending}
        >
          {integrityMutation.isPending ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <Shield className="mr-2 h-4 w-4" />}
          Run integrity check
        </Button>
        <Button variant="ghost" size="sm" onClick={() => queryClient.invalidateQueries({ queryKey: ["backup-list"] })}>
          <RefreshCw className="mr-2 h-3.5 w-3.5" /> Refresh
        </Button>
      </div>

      {/* Backup table */}
      <div className="rounded-lg border bg-white shadow-sm">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Created</TableHead>
              <TableHead className="text-right">Size</TableHead>
              <TableHead>Kind</TableHead>
              <TableHead>Verified</TableHead>
              <TableHead className="w-[120px]">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {backupQuery.data?.length === 0 && (
              <TableRow>
                <TableCell colSpan={6} className="py-8 text-center text-sm text-neutral-500">
                  No backups yet.
                </TableCell>
              </TableRow>
            )}
            {backupQuery.data?.map((b) => (
              <TableRow key={b.name}>
                <TableCell className="font-mono text-xs">{b.name}</TableCell>
                <TableCell className="text-sm">{formatDateTime(b.createdAt)}</TableCell>
                <TableCell className="text-right text-sm">{formatBytes(b.sizeBytes)}</TableCell>
                <TableCell>
                  <Badge variant={b.kind === "restore-safety" ? "warning" : "neutral"}>
                    {b.kind}
                  </Badge>
                </TableCell>
                <TableCell>
                  {b.verified
                    ? <Badge variant="success">OK</Badge>
                    : <Badge variant="danger">FAIL</Badge>
                  }
                </TableCell>
                <TableCell>
                  <div className="flex gap-1">
                    {canRestore && (
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-xs"
                        onClick={() => { setRestoreTarget(b); setRestoreConfirm(""); }}
                      >
                        <Undo2 className="mr-1 h-3 w-3" /> Restore
                      </Button>
                    )}
                    {canRestore && (
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-xs text-red-600 hover:text-red-700"
                        onClick={() => setDeleteTarget(b)}
                      >
                        <Trash2 className="mr-1 h-3 w-3" /> Delete
                      </Button>
                    )}
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>

      {/* Delete dialog */}
      <Dialog open={!!deleteTarget} onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete backup</DialogTitle>
            <DialogDescription>
              This will permanently remove <span className="font-mono">{deleteTarget?.name}</span>. This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>Cancel</Button>
            <Button
              variant="danger"
              disabled={deleteMutation.isPending}
              onClick={() => deleteTarget && deleteMutation.mutate(deleteTarget.name)}
            >
              {deleteMutation.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Restore dialog */}
      <Dialog open={!!restoreTarget} onOpenChange={(open) => { if (!open) { setRestoreTarget(null); setRestoreConfirm(""); } }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Restore backup</DialogTitle>
            <DialogDescription>
              This will create a safety snapshot, then schedule <span className="font-mono">{restoreTarget?.name}</span> to be restored on next app start. The application will need to be restarted.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label>Type the backup name to confirm:</Label>
            <Input
              placeholder={restoreTarget?.name ?? ""}
              value={restoreConfirm}
              onChange={(e) => setRestoreConfirm(e.target.value)}
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => { setRestoreTarget(null); setRestoreConfirm(""); }}>Cancel</Button>
            <Button
              disabled={restoreConfirm !== restoreTarget?.name || restoreMutation.isPending}
              variant="accent"
              onClick={() => restoreTarget && restoreMutation.mutate(restoreTarget.name)}
            >
              {restoreMutation.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Restore &amp; restart
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}