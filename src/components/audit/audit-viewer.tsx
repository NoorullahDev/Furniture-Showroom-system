"use client";

import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Loader2, RotateCcw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { PageHeader } from "@/components/page-header";
import {
  auditQuery,
  userList,
  type AuditEvent,
  type AuditFilter,
} from "@/lib/tauri/api";
import { isSessionError, useSession } from "@/components/session/session-provider";
import { asCommandError } from "@/lib/tauri/client";
import { formatDateTime } from "@/lib/format";

const LIMIT = 50;

export function AuditViewer() {
  const { refresh, hasPermission, profile } = useSession();

  const [action, setAction] = React.useState("");
  const [entityType, setEntityType] = React.useState("");
  const [userId, setUserId] = React.useState<number | null>(null);
  const [from, setFrom] = React.useState("");
  const [to, setTo] = React.useState("");
  const [offset, setOffset] = React.useState(0);
  const [applied, setApplied] = React.useState<AuditFilter>({ limit: LIMIT, offset: 0 });

  const [expanded, setExpanded] = React.useState<number | null>(null);

  const filter: AuditFilter = {
    action: action.trim() || null,
    entityType: entityType.trim() || null,
    userId: userId ?? null,
    from: from || null,
    to: to || null,
    limit: LIMIT,
    offset,
  };

  const query = useQuery({
    queryKey: ["audit", applied],
    queryFn: () => auditQuery(profile?.sessionId ?? "", applied),
    enabled: hasPermission("audit.view"),
  });

  const usersQuery = useQuery({
    queryKey: ["users"],
    queryFn: () => userList(profile?.sessionId ?? ""),
    enabled: hasPermission("audit.view") && hasPermission("user.manage"),
  });

  React.useEffect(() => {
    if (query.isError && isSessionError(query.error)) refresh();
  }, [query.isError, query.error, refresh]);

  function applyFilters() {
    setOffset(0);
    setApplied(filter);
  }

  function resetFilters() {
    setAction("");
    setEntityType("");
    setUserId(null);
    setFrom("");
    setTo("");
    setOffset(0);
    setApplied({ limit: LIMIT, offset: 0 });
  }

  function changePage(delta: number) {
    const next = Math.max(offset + delta, 0);
    setOffset(next);
    setApplied((a) => ({ ...a, offset: next }));
  }

  const userNames = React.useMemo(() => {
    const map = new Map<number, string>();
    for (const u of usersQuery.data ?? []) map.set(u.id, u.username);
    return map;
  }, [usersQuery.data]);

  if (!hasPermission("audit.view")) {
    return <PageHeader title="Audit log" subtitle="You do not have permission to view the audit log." />;
  }

  const total = query.data?.total ?? 0;
  const pageStart = offset + 1;
  const pageEnd = Math.min(offset + LIMIT, total);

  return (
    <div>
      <PageHeader
        title="Audit log"
        subtitle="Read-only record of security and data changes. Entries cannot be edited."
      />

      <div className="mt-5 rounded-lg border bg-white p-4 shadow-sm">
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <div className="grid gap-1.5">
            <Label htmlFor="af-action">Action</Label>
            <Input
              id="af-action"
              value={action}
              onChange={(e) => setAction(e.target.value)}
              placeholder="e.g. user.create"
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="af-entity">Entity type</Label>
            <Input
              id="af-entity"
              value={entityType}
              onChange={(e) => setEntityType(e.target.value)}
              placeholder="e.g. user"
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="af-user">User</Label>
            <select
              id="af-user"
              value={userId ?? ""}
              onChange={(e) => setUserId(e.target.value ? Number(e.target.value) : null)}
              className="h-9 w-full rounded-md border border-neutral-300 bg-white px-3 py-1 text-sm text-neutral-900 shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-forest-500/60"
            >
              <option value="">Any user</option>
              {usersQuery.data?.map((u) => (
                <option key={u.id} value={u.id}>
                  {u.username}
                </option>
              ))}
            </select>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div className="grid gap-1.5">
              <Label htmlFor="af-from">From</Label>
              <Input
                id="af-from"
                type="date"
                value={from}
                onChange={(e) => setFrom(e.target.value)}
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="af-to">To</Label>
              <Input id="af-to" type="date" value={to} onChange={(e) => setTo(e.target.value)} />
            </div>
          </div>
        </div>
        <div className="mt-3 flex items-center gap-2">
          <Button onClick={applyFilters}>Apply</Button>
          <Button variant="outline" onClick={resetFilters}>
            <RotateCcw className="h-3.5 w-3.5" />
            Reset
          </Button>
          <span className="ml-auto text-xs text-neutral-500">
            {total > 0
              ? `${pageStart}–${pageEnd} of ${total}`
              : query.data
                ? "No entries match"
                : ""}
          </span>
        </div>
      </div>

      <div className="mt-4">
        {query.isLoading && (
          <p className="flex items-center gap-2 text-sm text-neutral-500">
            <Loader2 className="h-4 w-4 animate-spin" /> Loading audit log…
          </p>
        )}
        {query.isError && !isSessionError(query.error) && (
          <p className="text-sm text-red-700">
            {asCommandError(query.error).message || "Failed to load audit log."}
          </p>
        )}
        {query.data && query.data.items.length > 0 && (
          <div className="rounded-lg border bg-white shadow-sm">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead />
                  <TableHead>Time</TableHead>
                  <TableHead>User</TableHead>
                  <TableHead>Action</TableHead>
                  <TableHead>Entity</TableHead>
                  <TableHead>Reason</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {query.data.items.map((event) => {
                  const open = expanded === event.id;
                  return (
                    <React.Fragment key={event.id}>
                      <TableRow className={open ? "bg-forest-50/50" : undefined}>
                        <TableCell className="w-8">
                          <button
                            type="button"
                            onClick={() => setExpanded(open ? null : event.id)}
                            aria-label={open ? "Collapse details" : "Show details"}
                            className="rounded p-1 text-neutral-500 hover:bg-neutral-100"
                          >
                            {open ? (
                              <ChevronDown className="h-4 w-4" />
                            ) : (
                              <ChevronRight className="h-4 w-4" />
                            )}
                          </button>
                        </TableCell>
                        <TableCell className="whitespace-nowrap text-xs text-neutral-500">
                          {formatDateTime(event.createdAt)}
                        </TableCell>
                        <TableCell className="text-xs">
                          {event.userId != null ? (userNames.get(event.userId) ?? event.userId) : "—"}
                        </TableCell>
                        <TableCell>
                          <Badge variant="neutral" className="font-mono">
                            {event.action}
                          </Badge>
                        </TableCell>
                        <TableCell className="text-xs text-neutral-600">
                          {event.entityType ? (
                            <span>
                              {event.entityType}
                              {event.entityId ? ` #${event.entityId}` : ""}
                            </span>
                          ) : (
                            "—"
                          )}
                        </TableCell>
                        <TableCell className="max-w-[12rem] truncate text-xs text-neutral-500">
                          {event.reason ?? "—"}
                        </TableCell>
                      </TableRow>
                      {open && (
                        <TableRow className="bg-forest-50/50">
                          <TableCell colSpan={6} className="p-3">
                            <AuditDetail event={event} />
                          </TableCell>
                        </TableRow>
                      )}
                    </React.Fragment>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}
        {query.data && query.data.items.length === 0 && (
          <p className="rounded-md border border-dashed border-neutral-300 bg-white p-6 text-center text-sm text-neutral-500">
            No audit entries match the current filters.
          </p>
        )}
      </div>

      {total > LIMIT && (
        <div className="mt-4 flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={offset === 0}
            onClick={() => changePage(-LIMIT)}
          >
            Previous
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={pageEnd >= total}
            onClick={() => changePage(LIMIT)}
          >
            Next
          </Button>
        </div>
      )}
    </div>
  );
}

function AuditDetail({ event }: { event: AuditEvent }) {
  return (
    <dl className="grid grid-cols-1 gap-2 text-xs sm:grid-cols-2">
      <div className="rounded-md border border-neutral-200 bg-white p-2">
        <dt className="font-medium text-neutral-500">Session</dt>
        <dd className="font-mono text-neutral-700">{event.sessionId ?? "—"}</dd>
      </div>
      <div className="rounded-md border border-neutral-200 bg-white p-2">
        <dt className="font-medium text-neutral-500">Correlation ID</dt>
        <dd className="font-mono text-neutral-700">{event.correlationId ?? "—"}</dd>
      </div>
      <JsonBlock label="Before" value={event.beforeJson} />
      <JsonBlock label="After" value={event.afterJson} />
    </dl>
  );
}

function JsonBlock({ label, value }: { label: string; value?: string | null }) {
  return (
    <div className="rounded-md border border-neutral-200 bg-white p-2">
      <dt className="font-medium text-neutral-500">{label}</dt>
      <dd>
        {value ? (
          <pre className="mt-1 max-h-32 overflow-auto font-mono text-[10px] leading-relaxed text-neutral-700">
            {value}
          </pre>
        ) : (
          <span className="text-neutral-400">—</span>
        )}
      </dd>
    </div>
  );
}