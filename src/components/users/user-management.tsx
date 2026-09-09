"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, Plus, UserPlus } from "lucide-react";

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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
import {
  roleList,
  userCreate,
  userDeactivate,
  userList,
  userResetPassword,
  userUpdate,
  type RoleDto,
  type UserDto,
} from "@/lib/tauri/api";
import { isSessionError, useSession } from "@/components/session/session-provider";
import { asCommandError, type CommandError } from "@/lib/tauri/client";
import { formatDateTime } from "@/lib/format";

function RoleCheckbox({
  role,
  checked,
  onChange,
  disabled,
}: {
  role: RoleDto;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label
      className={
        disabled
          ? "flex items-center gap-2 rounded-md border border-neutral-200 bg-neutral-50 px-3 py-2 text-sm opacity-60"
          : "flex cursor-pointer items-center gap-2 rounded-md border border-neutral-200 px-3 py-2 text-sm hover:border-forest-300"
      }
    >
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="h-4 w-4 rounded border-neutral-300 text-forest-600 focus:ring-forest-500"
      />
      <span className="grid gap-0 leading-tight">
        <span className="font-medium capitalize text-neutral-800">{role.name}</span>
        {role.description && (
          <span className="text-xs text-neutral-500">{role.description}</span>
        )}
      </span>
    </label>
  );
}

export function UserManagement() {
  const { toast } = useToast();
  const { refresh, hasPermission, profile } = useSession();
  const queryClient = useQueryClient();

  const usersQuery = useQuery({
    queryKey: ["users"],
    queryFn: () => userList(profile?.sessionId ?? ""),
    enabled: hasPermission("user.manage"),
  });

  const rolesQuery = useQuery({
    queryKey: ["roles"],
    queryFn: () => roleList(profile?.sessionId ?? ""),
    enabled: hasPermission("user.manage"),
  });

  const [creating, setCreating] = React.useState(false);
  const [editingUser, setEditingUser] = React.useState<UserDto | null>(null);
  const [resettingUser, setResettingUser] = React.useState<UserDto | null>(null);
  const [deactivatingUser, setDeactivatingUser] = React.useState<UserDto | null>(null);

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["users"] });
  };

  function handleError(e: unknown, title: string) {
    if (isSessionError(e)) {
      refresh();
      return;
    }
    const err = e as CommandError;
    toast({ variant: "error", title, description: err.message || "Unexpected error" });
  }

  const createMutation = useMutation({
    mutationFn: (input: {
      session: string;
      username: string;
      fullName: string;
      password: string;
      roles: string[];
    }) =>
      userCreate(input.session, {
        username: input.username,
        fullName: input.fullName,
        password: input.password,
        roles: input.roles,
      }),
    onSuccess: () => {
      toast({ variant: "success", title: "User created" });
      setCreating(false);
      invalidate();
    },
    onError: (e: unknown) => handleError(e, "Create failed"),
  });

  const updateMutation = useMutation({
    mutationFn: (input: {
      session: string;
      userId: number;
      fullName: string;
      isActive: boolean;
      roles: string[];
    }) =>
      userUpdate(input.session, {
        userId: input.userId,
        fullName: input.fullName,
        isActive: input.isActive,
        roles: input.roles,
      }),
    onSuccess: () => {
      toast({ variant: "success", title: "User updated" });
      setEditingUser(null);
      invalidate();
    },
    onError: (e: unknown) => handleError(e, "Update failed"),
  });

  const resetMutation = useMutation({
    mutationFn: (input: { session: string; userId: number; newPassword: string }) =>
      userResetPassword(input.session, input.userId, input.newPassword),
    onSuccess: () => {
      toast({ variant: "success", title: "Password reset" });
      setResettingUser(null);
    },
    onError: (e: unknown) => handleError(e, "Reset failed"),
  });

  const deactivateMutation = useMutation({
    mutationFn: (input: { session: string; userId: number; reason: string }) =>
      userDeactivate(input.session, input.userId, input.reason),
    onSuccess: () => {
      toast({ variant: "success", title: "User deactivated" });
      setDeactivatingUser(null);
      invalidate();
    },
    onError: (e: unknown) => handleError(e, "Deactivate failed"),
  });

  if (!hasPermission("user.manage")) {
    return (
      <PageHeader title="Users" subtitle="You do not have permission to view users." />
    );
  }

  return (
    <div>
      <PageHeader
        title="Users"
        subtitle="Local accounts. Passwords never leave the Rust backend."
        actions={
          hasPermission("user.create") ? (
            <Button onClick={() => setCreating(true)}>
              <Plus className="h-4 w-4" />
              New user
            </Button>
          ) : undefined
        }
      />

      <div className="mt-5">
        {usersQuery.isLoading && (
          <p className="flex items-center gap-2 text-sm text-neutral-500">
            <Loader2 className="h-4 w-4 animate-spin" /> Loading users…
          </p>
        )}
        {usersQuery.isError && (
          <p className="text-sm text-red-700">
            {asCommandError(usersQuery.error).message || "Failed to load users."}
          </p>
        )}
        {usersQuery.data && (
          <div className="rounded-lg border bg-white shadow-sm">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Username</TableHead>
                  <TableHead>Full name</TableHead>
                  <TableHead>Roles</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {usersQuery.data.map((user) => (
                  <TableRow key={user.id}>
                    <TableCell className="font-medium">{user.username}</TableCell>
                    <TableCell>{user.fullName}</TableCell>
                    <TableCell>
                      <div className="flex flex-wrap gap-1">
                        {user.roles.map((r) => (
                          <Badge key={r} variant="neutral" className="capitalize">
                            {r}
                          </Badge>
                        ))}
                      </div>
                    </TableCell>
                    <TableCell>
                      <Badge variant={user.isActive ? "success" : "danger"}>
                        {user.isActive ? "Active" : "Deactivated"}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-xs text-neutral-500">
                      {formatDateTime(user.createdAt)}
                    </TableCell>
                    <TableCell>
                      <div className="flex justify-end gap-1">
                        {hasPermission("user.edit") && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setEditingUser(user)}
                          >
                            Edit
                          </Button>
                        )}
                        {hasPermission("user.reset_password") && user.isActive && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setResettingUser(user)}
                          >
                            Reset password
                          </Button>
                        )}
                        {hasPermission("user.deactivate") && user.isActive && (
                          <Button
                            variant="ghost"
                            size="sm"
                            className="text-red-600 hover:bg-red-50"
                            disabled={user.id === profile?.userId}
                            title={
                              user.id === profile?.userId
                                ? "You cannot deactivate your own account"
                                : undefined
                            }
                            onClick={() => setDeactivatingUser(user)}
                          >
                            Deactivate
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>

      {creating && (
        <CreateUserDialog
          roles={rolesQuery.data ?? []}
          busy={createMutation.isPending}
          onCreate={(input) =>
            createMutation.mutate({
              session: profile?.sessionId ?? "",
              username: input.username,
              fullName: input.fullName,
              password: input.password,
              roles: input.roles,
            })
          }
          onClose={() => setCreating(false)}
        />
      )}

      {editingUser && (
        <EditUserDialog
          user={editingUser}
          roles={rolesQuery.data ?? []}
          busy={updateMutation.isPending}
          onSave={(fullName, isActive, roles) =>
            updateMutation.mutate({
              session: profile?.sessionId ?? "",
              userId: editingUser.id,
              fullName,
              isActive,
              roles,
            })
          }
          onClose={() => setEditingUser(null)}
        />
      )}

      {resettingUser && (
        <ResetPasswordDialog
          user={resettingUser}
          busy={resetMutation.isPending}
          onReset={(newPassword) =>
            resetMutation.mutate({
              session: profile?.sessionId ?? "",
              userId: resettingUser.id,
              newPassword,
            })
          }
          onClose={() => setResettingUser(null)}
        />
      )}

      {deactivatingUser && (
        <DeactivateUserDialog
          user={deactivatingUser}
          busy={deactivateMutation.isPending}
          onDeactivate={(reason) =>
            deactivateMutation.mutate({
              session: profile?.sessionId ?? "",
              userId: deactivatingUser.id,
              reason,
            })
          }
          onClose={() => setDeactivatingUser(null)}
        />
      )}
    </div>
  );
}

function CreateUserDialog({
  roles,
  busy,
  onCreate,
  onClose,
}: {
  roles: RoleDto[];
  busy: boolean;
  onCreate: (input: { username: string; fullName: string; password: string; roles: string[] }) => void;
  onClose: () => void;
}) {
  const [username, setUsername] = React.useState("");
  const [fullName, setFullName] = React.useState("");
  const [password, setPassword] = React.useState("");
  const [selected, setSelected] = React.useState<string[]>([]);
  const [error, setError] = React.useState<string | null>(null);

  const toggle = (code: string) =>
    setSelected((s) => (s.includes(code) ? s.filter((c) => c !== code) : [...s, code]));

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const cleanUsername = username.trim().toLowerCase();
    if (cleanUsername.length < 3 || !/^[a-z0-9._-]+$/.test(cleanUsername)) {
      setError("Username must be 3+ characters: letters, digits, '.', '_' or '-'.");
      return;
    }
    if (!fullName.trim()) {
      setError("Full name is required.");
      return;
    }
    if (password.length < 8 || !/[A-Za-z]/.test(password) || !/\d/.test(password)) {
      setError("Password must be 8+ characters with at least one letter and one digit.");
      return;
    }
    if (selected.length === 0) {
      setError("Select at least one role.");
      return;
    }
    setError(null);
    onCreate({ username: cleanUsername, fullName: fullName.trim(), password, roles: selected });
  }

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New user</DialogTitle>
          <DialogDescription>Create a local account and assign roles.</DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="grid gap-1.5">
              <Label htmlFor="nu-username">Username</Label>
              <Input
                id="nu-username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                autoFocus
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="nu-fullName">Full name</Label>
              <Input
                id="nu-fullName"
                value={fullName}
                onChange={(e) => setFullName(e.target.value)}
              />
            </div>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="nu-password">Password</Label>
            <Input
              id="nu-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="new-password"
            />
            <p className="flex items-center gap-1 text-xs text-neutral-500">
              <UserPlus className="h-3.5 w-3.5" />
              Min 8 characters, at least one letter and one digit.
            </p>
          </div>
          <div className="grid gap-2">
            <Label>Roles</Label>
            {roles.map((role) => (
              <RoleCheckbox
                key={role.code}
                role={role}
                checked={selected.includes(role.code)}
                onChange={() => toggle(role.code)}
              />
            ))}
          </div>
          {error && (
            <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              Create user
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function EditUserDialog({
  user,
  roles,
  busy,
  onSave,
  onClose,
}: {
  user: UserDto;
  roles: RoleDto[];
  busy: boolean;
  onSave: (fullName: string, isActive: boolean, roles: string[]) => void;
  onClose: () => void;
}) {
  const [fullName, setFullName] = React.useState(user.fullName);
  const [isActive, setIsActive] = React.useState(user.isActive);
  const [selected, setSelected] = React.useState<string[]>(user.roles);
  const [error, setError] = React.useState<string | null>(null);

  const toggle = (code: string) =>
    setSelected((s) => (s.includes(code) ? s.filter((c) => c !== code) : [...s, code]));

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!fullName.trim()) {
      setError("Full name is required.");
      return;
    }
    if (selected.length === 0) {
      setError("Select at least one role.");
      return;
    }
    setError(null);
    onSave(fullName.trim(), isActive, selected);
  }

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Edit {user.username}</DialogTitle>
          <DialogDescription>Update the display name, status, and roles.</DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid gap-1.5">
            <Label htmlFor="eu-fullName">Full name</Label>
            <Input
              id="eu-fullName"
              value={fullName}
              onChange={(e) => setFullName(e.target.value)}
              autoFocus
            />
          </div>
          <label className="flex items-center gap-2 text-sm text-neutral-800">
            <input
              type="checkbox"
              checked={isActive}
              onChange={(e) => setIsActive(e.target.checked)}
              className="h-4 w-4 rounded border-neutral-300 text-forest-600 focus:ring-forest-500"
            />
            Account is active
          </label>
          <div className="grid gap-2">
            <Label>Roles</Label>
            {roles.map((role) => (
              <RoleCheckbox
                key={role.code}
                role={role}
                checked={selected.includes(role.code)}
                onChange={() => toggle(role.code)}
              />
            ))}
          </div>
          {error && (
            <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              Save
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ResetPasswordDialog({
  user,
  busy,
  onReset,
  onClose,
}: {
  user: UserDto;
  busy: boolean;
  onReset: (newPassword: string) => void;
  onClose: () => void;
}) {
  const [password, setPassword] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (password.length < 8 || !/[A-Za-z]/.test(password) || !/\d/.test(password)) {
      setError("Password must be 8+ characters with at least one letter and one digit.");
      return;
    }
    if (password !== confirm) {
      setError("Passwords do not match.");
      return;
    }
    setError(null);
    onReset(password);
  }

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Reset password for {user.username}</DialogTitle>
          <DialogDescription>
            Live sessions for this user will be signed out immediately.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid gap-1.5">
            <Label htmlFor="rp-password">New password</Label>
            <Input
              id="rp-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="new-password"
              autoFocus
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="rp-confirm">Confirm new password</Label>
            <Input
              id="rp-confirm"
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              autoComplete="new-password"
            />
          </div>
          {error && (
            <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              Reset password
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function DeactivateUserDialog({
  user,
  busy,
  onDeactivate,
  onClose,
}: {
  user: UserDto;
  busy: boolean;
  onDeactivate: (reason: string) => void;
  onClose: () => void;
}) {
  const [reason, setReason] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!reason.trim()) {
      setError("A reason is required for deactivation.");
      return;
    }
    setError(null);
    onDeactivate(reason.trim());
  }

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Deactivate {user.username}?</DialogTitle>
          <DialogDescription>
            {user.fullName} will lose signing access and all live sessions will be revoked. Their
            history is preserved.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid gap-1.5">
            <Label htmlFor="du-reason">Reason (required)</Label>
            <Input
              id="du-reason"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="e.g. Left the shop"
              autoFocus
            />
          </div>
          {error && (
            <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" variant="danger" disabled={busy}>
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              Deactivate
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}