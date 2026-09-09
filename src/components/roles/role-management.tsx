"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, Pencil } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/components/page-header";
import { roleList, rolePermissionsSet, type RoleDto } from "@/lib/tauri/api";
import { PERMISSION_GROUPS } from "@/lib/permissions";
import { isSessionError, useSession } from "@/components/session/session-provider";
import { asCommandError, type CommandError } from "@/lib/tauri/client";

export function RoleManagement() {
  const { toast } = useToast();
  const { refresh, hasPermission, profile } = useSession();
  const queryClient = useQueryClient();

  const rolesQuery = useQuery({
    queryKey: ["roles"],
    queryFn: () => roleList(profile?.sessionId ?? ""),
    enabled: hasPermission("user.manage"),
  });

  const [editing, setEditing] = React.useState<RoleDto | null>(null);

  const saveMutation = useMutation({
    mutationFn: (input: { roleId: number; permissions: string[] }) =>
      rolePermissionsSet(profile?.sessionId ?? "", input.roleId, input.permissions),
    onSuccess: () => {
      toast({ variant: "success", title: "Permissions saved" });
      setEditing(null);
      void queryClient.invalidateQueries({ queryKey: ["roles"] });
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      const err = e as CommandError;
      toast({ variant: "error", title: "Save failed", description: err.message });
    },
  });

  if (!hasPermission("user.manage")) {
    return <PageHeader title="Roles" subtitle="You do not have permission to view roles." />;
  }

  return (
    <div>
      <PageHeader
        title="Roles & permissions"
        subtitle="Role templates are editable; permission checks always run in Rust."
      />

      {rolesQuery.isLoading && (
        <p className="mt-5 flex items-center gap-2 text-sm text-neutral-500">
          <Loader2 className="h-4 w-4 animate-spin" /> Loading roles…
        </p>
      )}
      {rolesQuery.isError && (
        <p className="mt-5 text-sm text-red-700">
          {asCommandError(rolesQuery.error).message || "Failed to load roles."}
        </p>
      )}
      {rolesQuery.data && (
        <div className="mt-5 rounded-lg border bg-white shadow-sm">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Role</TableHead>
                <TableHead>Description</TableHead>
                <TableHead>Type</TableHead>
                <TableHead className="text-right">Permissions</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rolesQuery.data.map((role) => (
                <TableRow key={role.id}>
                  <TableCell className="font-medium capitalize">{role.name}</TableCell>
                  <TableCell className="text-sm text-neutral-500">
                    {role.description ?? "—"}
                  </TableCell>
                  <TableCell>
                    <Badge variant={role.isSystem ? "info" : "neutral"}>
                      {role.isSystem ? "Template" : "Custom"}
                    </Badge>
                  </TableCell>
                  <TableCell className="tabular-nums text-right">
                    {role.permissions.length}
                  </TableCell>
                  <TableCell>
                    <div className="flex justify-end gap-1">
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={!hasPermission("role.manage")}
                        title={
                          hasPermission("role.manage") ? undefined : "Requires role.manage"
                        }
                        onClick={() => setEditing(role)}
                      >
                        <Pencil className="h-3.5 w-3.5" />
                        Edit
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}

      {editing && (
        <RoleEditorDialog
          role={editing}
          busy={saveMutation.isPending}
          onSave={(permissions) =>
            saveMutation.mutate({ roleId: editing.id, permissions })
          }
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  );
}

function RoleEditorDialog({
  role,
  busy,
  onSave,
  onClose,
}: {
  role: RoleDto;
  busy: boolean;
  onSave: (permissions: string[]) => void;
  onClose: () => void;
}) {
  const [selected, setSelected] = React.useState<string[]>(role.permissions);
  const [allOpen, setAllOpen] = React.useState(true);

  const toggle = (code: string) =>
    setSelected((s) => (s.includes(code) ? s.filter((c) => c !== code) : [...s, code]));

  const selectedCount = PERMISSION_GROUPS.reduce(
    (sum, group) =>
      sum +
      group.permissions.reduce(
        (n, p) => n + (selected.includes(p.code) ? 1 : 0),
        0,
      ),
    0,
  );

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent className="max-w-2xl sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle className="capitalize">
            {role.name} — permissions
          </DialogTitle>
          <DialogDescription>
            {selectedCount} of {PERMISSION_GROUPS.flatMap((g) => g.permissions).length} selected.
            New permissions apply to this role immediately.
          </DialogDescription>
        </DialogHeader>

        <div className="max-h-[60vh] space-y-4 overflow-y-auto pr-1">
          {PERMISSION_GROUPS.map((group) => {
            const groupSelected = group.permissions.filter((p) =>
              selected.includes(p.code),
            ).length;
            const groupTotal = group.permissions.length;
            const allChecked = groupSelected === groupTotal;
            const someChecked = groupSelected > 0 && !allChecked;

            return (
              <fieldset key={group.group} className="rounded-md border border-neutral-200 p-3">
                <legend className="flex w-full items-center justify-between px-1">
                  <div className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={allChecked}
                      ref={(el) => {
                        if (el) el.indeterminate = someChecked;
                      }}
                      onChange={() => {
                        setSelected((s) => {
                          const codes = group.permissions.map((p) => p.code);
                          const next = new Set(s);
                          if (allChecked) {
                            codes.forEach((c) => next.delete(c));
                          } else {
                            codes.forEach((c) => next.add(c));
                          }
                          return [...next];
                        });
                      }}
                      className="h-4 w-4 rounded border-neutral-300 text-forest-600 focus:ring-forest-500"
                      aria-label={`Toggle all ${group.group}`}
                    />
                    <span className="text-sm font-semibold text-neutral-800">
                      {group.group}
                    </span>
                    <span className="text-xs text-neutral-400">
                      {groupSelected}/{groupTotal}
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={() => setAllOpen((v) => !v)}
                    className="text-xs text-forest-600 hover:underline"
                  >
                    {allOpen ? "Collapse" : "Expand"}
                  </button>
                </legend>
                {allOpen && (
                  <div className="mt-2 grid grid-cols-1 gap-1 sm:grid-cols-2">
                    {group.permissions.map((perm) => {
                      const checked = selected.includes(perm.code);
                      return (
                        <label
                          key={perm.code}
                          className={
                            checked
                              ? "flex cursor-pointer items-start gap-2 rounded-md bg-forest-50 px-2 py-1.5"
                              : "flex cursor-pointer items-start gap-2 rounded-md px-2 py-1.5 hover:bg-neutral-50"
                          }
                        >
                          <input
                            type="checkbox"
                            checked={checked}
                            onChange={() => toggle(perm.code)}
                            className="mt-0.5 h-4 w-4 rounded border-neutral-300 text-forest-600 focus:ring-forest-500"
                          />
                          <span className="grid gap-0 leading-tight">
                            <span className="text-sm font-medium text-neutral-800">
                              {perm.label}
                            </span>
                            <span className="text-xs text-neutral-500">{perm.description}</span>
                            <code className="text-[10px] text-neutral-400">{perm.code}</code>
                          </span>
                        </label>
                      );
                    })}
                  </div>
                )}
              </fieldset>
            );
          })}
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button
            type="button"
            onClick={() => onSave(selected)}
            disabled={busy || role.permissions.length === selected.length &&
              role.permissions.every((p) => selected.includes(p))}
          >
            {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
            Save permissions
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}