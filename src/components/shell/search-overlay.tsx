"use client";

import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import { CornerDownLeft, Loader2, Search } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { useSession } from "@/components/session/session-provider";
import {
  globalSearch,
  type SearchResultDto,
  type SearchResultKind,
} from "@/lib/tauri/api";
import { asCommandError } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";
import type { ShellView } from "@/lib/shell";

const KIND_META: Record<SearchResultKind, { label: string; view: ShellView }> = {
  product: { label: "Product", view: "catalogue" },
  customer: { label: "Customer", view: "sales" },
  supplier: { label: "Supplier", view: "purchases" },
  sale: { label: "Sale", view: "sales" },
  purchase: { label: "Purchase", view: "purchases" },
  supplier_payment: { label: "Supplier payment", view: "purchases" },
  delivery: { label: "Delivery", view: "fulfilment" },
  receipt: { label: "Receipt", view: "sales" },
  expense: { label: "Expense", view: "finance" },
};

const MIN_QUERY = 2;

export function SearchOverlay({
  open,
  onOpenChange,
  onNavigate,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onNavigate: (view: ShellView) => void;
}) {
  const { profile } = useSession();
  const session = profile?.sessionId ?? "";

  const [input, setInput] = React.useState("");
  const [query, setQuery] = React.useState("");
  const [activeIndex, setActiveIndex] = React.useState(0);

  React.useEffect(() => {
    if (open) {
      setInput("");
      setQuery("");
      setActiveIndex(0);
    }
  }, [open]);

  React.useEffect(() => {
    const trimmed = input.trim();
    if (trimmed.length < MIN_QUERY) {
      setQuery("");
      return;
    }
    const id = window.setTimeout(() => setQuery(trimmed), 250);
    return () => window.clearTimeout(id);
  }, [input]);

  const searchQuery = useQuery({
    queryKey: ["search", "global", query],
    queryFn: () => globalSearch(session, query),
    enabled: !!session && query.trim().length >= MIN_QUERY,
  });

  const results = searchQuery.data?.results ?? [];

  React.useEffect(() => {
    setActiveIndex(0);
  }, [results.length]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, Math.max(0, results.length - 1)));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const hit = results[activeIndex];
      if (!hit) return;
      onNavigate(KIND_META[hit.kind].view);
      onOpenChange(false);
    }
  };

  const error = searchQuery.isError
    ? asCommandError(searchQuery.error).message
    : null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTitle className="sr-only">Search everything</DialogTitle>
      <DialogContent
        className="max-w-xl gap-0 overflow-hidden p-0"
        onKeyDown={onKeyDown}
      >
        <div className="flex items-center gap-2.5 border-b border-neutral-100 px-4 py-3">
          {searchQuery.isFetching ? (
            <Loader2 className="h-4 w-4 shrink-0 animate-spin text-forest-600" />
          ) : (
            <Search className="h-4 w-4 shrink-0 text-neutral-400" />
          )}
          <Input
            autoFocus
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              setActiveIndex(0);
            }}
            placeholder="Search products, customers, invoices, receipts, deliveries…"
            className="border-0 px-0 shadow-none focus-visible:ring-0 focus-visible:border-transparent"
          />
        </div>

        <div className="max-h-[380px] overflow-y-auto py-2">
          {query.length < MIN_QUERY && (
            <p className="px-4 py-8 text-center text-sm text-neutral-400">
              Type at least {MIN_QUERY} characters to search everything.
            </p>
          )}
          {query.length >= MIN_QUERY && searchQuery.isLoading && (
            <p className="px-4 py-8 text-center text-sm text-neutral-400">Searching…</p>
          )}
          {error && (
            <p className="px-4 py-8 text-center text-sm text-red-600">{error}</p>
          )}
          {query.length >= MIN_QUERY && !searchQuery.isLoading && !error && results.length === 0 && (
            <p className="px-4 py-8 text-center text-sm text-neutral-400">
              No results for “{query}”.
            </p>
          )}
          {results.map((hit, idx) => (
            <SearchHit
              key={`${hit.kind}:${hit.id}`}
              hit={hit}
              active={idx === activeIndex}
              onSelect={() => {
                onNavigate(KIND_META[hit.kind].view);
                onOpenChange(false);
              }}
              onHover={() => setActiveIndex(idx)}
            />
          ))}
        </div>

        {results.length > 0 && (
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-neutral-100 px-4 py-2.5 text-[11px] text-neutral-400">
            <span className="flex items-center gap-1">
              <CornerDownLeft className="h-3 w-3" /> to open
            </span>
            <span>
              <kbd className="font-sans">↑</kbd>/<kbd className="font-sans">↓</kbd> to navigate
            </span>
            <span>Esc to close</span>
            <span className="ml-auto">{results.length} result(s)</span>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function SearchHit({
  hit,
  active,
  onSelect,
  onHover,
}: {
  hit: SearchResultDto;
  active: boolean;
  onSelect: () => void;
  onHover: () => void;
}) {
  const meta = KIND_META[hit.kind];
  return (
    <button
      type="button"
      onClick={onSelect}
      onMouseEnter={onHover}
      className={cn(
        "mx-2 flex w-[calc(100%-1rem)] items-center gap-3 rounded-md px-2 py-2 text-left",
        active && "bg-forest-50",
      )}
    >
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-neutral-800">{hit.title}</span>
          {hit.rank === 0 && (
            <Badge variant="info" className="shrink-0 px-1.5 text-[10px]">
              Exact
            </Badge>
          )}
        </span>
        {hit.subtitle || hit.refNumber ? (
          <span className="block truncate text-xs text-neutral-500">
            {hit.refNumber ? (
              <span className="font-mono">{hit.refNumber}</span>
            ) : null}
            {hit.refNumber && hit.subtitle ? " · " : null}
            {hit.subtitle}
          </span>
        ) : null}
      </span>
      <span className="shrink-0 text-xs font-medium text-forest-700">
        Open in {meta.label}
      </span>
    </button>
  );
}