"use client";

import * as React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  FileUp,
  Loader2,
  Pencil,
  Plus,
  RotateCcw,
  Search,
  Tags,
  Undo2,
  Wallet,
} from "lucide-react";

import { PageHeader } from "@/components/page-header";
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
import { MoneyInput } from "@/components/ui/money-input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useSession, isSessionError } from "@/components/session/session-provider";
import { useToast } from "@/components/ui/toast";
import { formatCurrency } from "@/lib/format";
import {
  cashAccountList,
  expenseCategoryCreate,
  expenseCategoryList,
  expenseCategoryUpdate,
  expensePage,
  expensePost,
  expenseReverse,
  ownerTransactionList,
  ownerTransactionPost,
  paymentMethodList,
  profitSummary,
  settingsGet,
  type CashAccountDto,
  type ExpenseCategoryDto,
  type ExpenseDto,
  type PaymentMethodDto,
  type ProfitSummaryDto,
} from "@/lib/tauri/api";
import { commandErrorMessage } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";

type SortKey = "date" | "category" | "note" | "amount";
type SortDirection = "asc" | "desc";
type DateRange = { from: string; to: string };

const PAGE_SIZE = 20;

function localIso(date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function datePreset(kind: "today" | "yesterday" | "week" | "month" | "lastMonth"): DateRange {
  const now = new Date();
  const from = new Date(now);
  const to = new Date(now);
  if (kind === "yesterday") {
    from.setDate(from.getDate() - 1);
    to.setDate(to.getDate() - 1);
  } else if (kind === "week") {
    const mondayOffset = (now.getDay() + 6) % 7;
    from.setDate(from.getDate() - mondayOffset);
  } else if (kind === "month") {
    from.setDate(1);
  } else if (kind === "lastMonth") {
    from.setMonth(from.getMonth() - 1, 1);
    to.setDate(0);
  }
  return { from: localIso(from), to: localIso(to) };
}

function readableDate(value: string): string {
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString("en-PK", { day: "2-digit", month: "short", year: "numeric" });
}

function configuredCurrency(raw?: string | null): string {
  if (!raw) return "PKR";
  try {
    const parsed = JSON.parse(raw) as unknown;
    return typeof parsed === "string" && /^[A-Z]{3}$/i.test(parsed) ? parsed.toUpperCase() : "PKR";
  } catch {
    return "PKR";
  }
}

function categoryCode(name: string): string {
  return name
    .trim()
    .toUpperCase()
    .replace(/[^A-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 40) || `EXPENSE_${Date.now()}`;
}

function idempotencyKey(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) return `expense-${crypto.randomUUID()}`;
  return `expense-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function ExpensesPage() {
  const { profile, refresh, hasPermission } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";
  const canView = hasPermission("expense.view");
  const canCreate = hasPermission("expense.create");
  const canReverse = hasPermission("expense.reverse");
  const canOwner = hasPermission("owner.transfer");
  const canProfit = hasPermission("profit.view");

  const [search, setSearch] = React.useState("");
  const deferredSearch = React.useDeferredValue(search);
  const [categoryId, setCategoryId] = React.useState<number | null>(null);
  const [fromDate, setFromDate] = React.useState("");
  const [toDate, setToDate] = React.useState("");
  const [page, setPage] = React.useState(0);
  const [sortBy, setSortBy] = React.useState<SortKey>("date");
  const [sortDirection, setSortDirection] = React.useState<SortDirection>("desc");
  const [addOpen, setAddOpen] = React.useState(false);
  const [categoriesOpen, setCategoriesOpen] = React.useState(false);
  const [reverseTarget, setReverseTarget] = React.useState<ExpenseDto | null>(null);
  const [financeTool, setFinanceTool] = React.useState<"owner" | "profit" | null>(null);

  React.useEffect(() => setPage(0), [deferredSearch, categoryId, fromDate, toDate]);

  const categoriesQuery = useQuery({
    queryKey: ["expenses", "categories"],
    queryFn: () => expenseCategoryList(session),
    enabled: !!session && canView,
  });
  const accountsQuery = useQuery({
    queryKey: ["expenses", "accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session && canView,
  });
  const methodsQuery = useQuery({
    queryKey: ["expenses", "payment-methods"],
    queryFn: () => paymentMethodList(session),
    enabled: !!session && canView,
  });
  const currencyQuery = useQuery({
    queryKey: ["settings", "shop.currency"],
    queryFn: () => settingsGet(session, "shop.currency"),
    enabled: !!session,
  });
  const expensesQuery = useQuery({
    queryKey: ["expenses", "page", deferredSearch, categoryId, fromDate, toDate, page, sortBy, sortDirection],
    queryFn: () => expensePage(session, {
      search: deferredSearch.trim() || null,
      categoryId,
      fromDate: fromDate || null,
      toDate: toDate || null,
      sortBy,
      sortDirection,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    }),
    enabled: !!session && canView,
    placeholderData: (previous) => previous,
  });

  const currency = configuredCurrency(currencyQuery.data);
  const categories = categoriesQuery.data ?? [];
  const accounts = accountsQuery.data ?? [];
  const methods = methodsQuery.data ?? [];
  const totalPages = Math.max(1, Math.ceil((expensesQuery.data?.total ?? 0) / PAGE_SIZE));

  React.useEffect(() => {
    if (page >= totalPages) setPage(totalPages - 1);
  }, [page, totalPages]);

  const invalidate = React.useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: ["expenses"] });
    void queryClient.invalidateQueries({ queryKey: ["reports"] });
    void queryClient.invalidateQueries({ queryKey: ["purchasing"] });
  }, [queryClient]);

  const handleError = React.useCallback((error: Error, title = "Operation failed") => {
    if (isSessionError(error)) {
      void refresh();
      return;
    }
    toast({ variant: "error", title, description: commandErrorMessage(error) });
  }, [refresh, toast]);

  const setPreset = (kind: Parameters<typeof datePreset>[0]) => {
    const range = datePreset(kind);
    setFromDate(range.from);
    setToDate(range.to);
  };

  const toggleSort = (key: SortKey) => {
    if (sortBy === key) setSortDirection((current) => current === "asc" ? "desc" : "asc");
    else {
      setSortBy(key);
      setSortDirection(key === "date" ? "desc" : "asc");
    }
    setPage(0);
  };

  if (!canView) {
    return <StatePanel message="You do not have permission to view expenses." />;
  }

  const pageData = expensesQuery.data;
  const hasFilters = !!(search || categoryId || fromDate || toDate);

  return (
    <div className="min-h-full">
      <PageHeader
        title="Expenses"
        subtitle={pageData
          ? `${pageData.total.toLocaleString()} ${pageData.total === 1 ? "expense" : "expenses"} · ${formatCurrency(pageData.totalAmountMinor, currency)} total`
          : "Loading expense totals…"}
        actions={
          <>
            {canCreate && (
              <Button variant="outline" onClick={() => setCategoriesOpen(true)}>
                <Tags className="h-4 w-4" /> Manage Categories
              </Button>
            )}
            {canCreate && (
              <Button onClick={() => setAddOpen(true)}>
                <Plus className="h-4 w-4" /> Add Expense
              </Button>
            )}
          </>
        }
      />

      <section className="mt-6 overflow-hidden rounded-xl border border-neutral-200 bg-white shadow-sm">
        <div className="grid gap-3 border-b border-neutral-200 p-3 xl:grid-cols-[minmax(220px,1fr)_200px_170px_170px_auto] xl:items-center">
          <div className="relative">
            <Search className="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-neutral-400" />
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              className="pl-9"
              placeholder="Search note, category, or bill reference"
              aria-label="Search expenses"
            />
          </div>
          <Select value={categoryId === null ? "all" : String(categoryId)} onValueChange={(value) => setCategoryId(value === "all" ? null : Number(value))}>
            <SelectTrigger aria-label="Filter by category"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All categories</SelectItem>
              {categories.map((category) => <SelectItem key={category.id} value={String(category.id)}>{category.name}{category.isActive ? "" : " (archived)"}</SelectItem>)}
            </SelectContent>
          </Select>
          <Input type="date" value={fromDate} max={toDate || undefined} onChange={(event) => setFromDate(event.target.value)} aria-label="Start date" />
          <Input type="date" value={toDate} min={fromDate || undefined} onChange={(event) => setToDate(event.target.value)} aria-label="End date" />
          <div className="flex flex-wrap gap-1.5 xl:justify-end">
            <PresetButton label="Today" onClick={() => setPreset("today")} />
            <PresetButton label="Yesterday" onClick={() => setPreset("yesterday")} />
            <PresetButton label="This Week" onClick={() => setPreset("week")} />
            <PresetButton label="This Month" onClick={() => setPreset("month")} />
            <PresetButton label="Last Month" onClick={() => setPreset("lastMonth")} />
            {hasFilters && <Button variant="ghost" size="sm" title="Clear filters" onClick={() => { setSearch(""); setCategoryId(null); setFromDate(""); setToDate(""); }}><RotateCcw className="h-3.5 w-3.5" /></Button>}
          </div>
        </div>

        {expensesQuery.isLoading ? (
          <StatePanel loading message="Loading expenses…" />
        ) : expensesQuery.isError ? (
          <StatePanel
            error
            message={commandErrorMessage(expensesQuery.error as Error)}
            action={<Button variant="outline" size="sm" onClick={() => void expensesQuery.refetch()}>Try again</Button>}
          />
        ) : !pageData?.items.length ? (
          <StatePanel message={hasFilters ? "No expenses match the selected filters." : "No expenses recorded yet."} />
        ) : (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <SortableHead label="Date" sortKey="date" active={sortBy} direction={sortDirection} onSort={toggleSort} />
                  <SortableHead label="Category" sortKey="category" active={sortBy} direction={sortDirection} onSort={toggleSort} />
                  <SortableHead label="Note" sortKey="note" active={sortBy} direction={sortDirection} onSort={toggleSort} />
                  <SortableHead label="Amount" sortKey="amount" active={sortBy} direction={sortDirection} onSort={toggleSort} right />
                  <TableHead className="w-20 text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pageData.items.map((expense) => (
                  <TableRow key={expense.id} className={cn(expense.status === "reversed" && "bg-neutral-50 text-neutral-500")}>
                    <TableCell className="whitespace-nowrap">{readableDate(expense.expenseDate)}</TableCell>
                    <TableCell className="font-medium text-neutral-800">{expense.categoryName}</TableCell>
                    <TableCell className="max-w-[520px]">
                      <div className="truncate text-neutral-700">{expense.description || "—"}</div>
                      <div className="mt-0.5 flex flex-wrap gap-x-2 text-xs text-neutral-400">
                        {expense.reference && <span>Ref: {expense.reference}</span>}
                        <span>{expense.expenseNumber ?? `#${expense.id}`}</span>
                        <span>{expense.paymentMethodName ?? "Legacy"} · {expense.cashAccountName}</span>
                        {expense.attachmentPath && <span title={expense.attachmentPath}>Bill attached</span>}
                        {expense.status === "reversed" && <Badge variant="warning">Reversed</Badge>}
                      </div>
                      {expense.reversalReason && <div className="mt-1 text-xs text-rose-600">Reason: {expense.reversalReason}</div>}
                    </TableCell>
                    <TableCell className={cn("text-right font-semibold tabular-nums", expense.status === "reversed" ? "line-through text-neutral-400" : "text-neutral-900")}>
                      {formatCurrency(expense.amountMinor, currency)}
                    </TableCell>
                    <TableCell className="text-right">
                      {canReverse && expense.status === "posted" ? (
                        <Button variant="ghost" size="icon" title="Reverse expense" aria-label={`Reverse ${expense.expenseNumber ?? expense.id}`} onClick={() => setReverseTarget(expense)}>
                          <Undo2 className="h-4 w-4" />
                        </Button>
                      ) : <span className="text-neutral-300">—</span>}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}

        <div className="flex items-center justify-between border-t border-neutral-200 px-4 py-3 text-sm text-neutral-500">
          <span>{pageData?.total ? `Showing ${page * PAGE_SIZE + 1}–${Math.min((page + 1) * PAGE_SIZE, pageData.total)} of ${pageData.total}` : "0 expenses"}</span>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="icon" aria-label="Previous page" disabled={page === 0 || expensesQuery.isFetching} onClick={() => setPage((value) => Math.max(0, value - 1))}><ChevronLeft className="h-4 w-4" /></Button>
            <span>Page {page + 1} of {totalPages}</span>
            <Button variant="outline" size="icon" aria-label="Next page" disabled={page + 1 >= totalPages || expensesQuery.isFetching} onClick={() => setPage((value) => value + 1)}><ChevronRight className="h-4 w-4" /></Button>
          </div>
        </div>
      </section>

      {(canOwner || canProfit) && (
        <details className="mt-4 rounded-lg border border-neutral-200 bg-white p-4">
          <summary className="cursor-pointer text-sm font-medium text-neutral-700">Other finance tools</summary>
          <div className="mt-4 flex gap-2 border-b border-neutral-200 pb-3">
            {canOwner && <Button variant={financeTool === "owner" ? "secondary" : "outline"} size="sm" onClick={() => setFinanceTool("owner")}><Wallet className="h-4 w-4" /> Owner transfers</Button>}
            {canProfit && <Button variant={financeTool === "profit" ? "secondary" : "outline"} size="sm" onClick={() => setFinanceTool("profit")}>Profit summary</Button>}
          </div>
          {financeTool === "owner" && canOwner && <OwnerTransfers session={session} accounts={accounts} currency={currency} onError={handleError} />}
          {financeTool === "profit" && canProfit && <ProfitSummary session={session} currency={currency} />}
        </details>
      )}

      {addOpen && canCreate && (
        <AddExpenseDialog
          session={session}
          categories={categories}
          accounts={accounts}
          methods={methods}
          currency={currency}
          onClose={() => setAddOpen(false)}
          onSaved={() => { setAddOpen(false); invalidate(); toast({ variant: "success", title: "Expense added" }); }}
          onError={handleError}
        />
      )}
      {categoriesOpen && canCreate && (
        <CategoryManagerDialog
          session={session}
          categories={categories}
          loading={categoriesQuery.isLoading}
          onClose={() => setCategoriesOpen(false)}
          onChanged={invalidate}
          onError={handleError}
        />
      )}
      {reverseTarget && canReverse && (
        <ReverseExpenseDialog
          session={session}
          expense={reverseTarget}
          currency={currency}
          onClose={() => setReverseTarget(null)}
          onSaved={() => { setReverseTarget(null); invalidate(); toast({ variant: "success", title: "Expense reversed" }); }}
          onError={handleError}
        />
      )}
    </div>
  );
}

function PresetButton({ label, onClick }: { label: string; onClick: () => void }) {
  return <Button variant="outline" size="sm" className="rounded-full px-3" onClick={onClick}>{label}</Button>;
}

function SortableHead({ label, sortKey, active, direction, onSort, right }: { label: string; sortKey: SortKey; active: SortKey; direction: SortDirection; onSort: (key: SortKey) => void; right?: boolean }) {
  const Icon = active === sortKey && direction === "asc" ? ArrowUp : ArrowDown;
  return (
    <TableHead className={right ? "text-right" : undefined}>
      <button type="button" className={cn("inline-flex items-center gap-1", right && "ml-auto")} onClick={() => onSort(sortKey)}>
        {label}<Icon className={cn("h-3.5 w-3.5", active !== sortKey && "opacity-25")} />
      </button>
    </TableHead>
  );
}

function StatePanel({ message, loading, error, action }: { message: string; loading?: boolean; error?: boolean; action?: React.ReactNode }) {
  return (
    <div className={cn("flex min-h-64 flex-col items-center justify-center gap-3 p-10 text-center text-sm", error ? "text-rose-600" : "text-neutral-500")}>
      {loading && <Loader2 className="h-5 w-5 animate-spin" />}
      <span>{message}</span>{action}
    </div>
  );
}

type DialogActions = { session: string; onClose: () => void; onSaved: () => void; onError: (error: Error, title?: string) => void };

function ExpenseFormDialog({ title, description, busy, valid, onSubmit, onClose, children, submitLabel }: { title: string; description: string; busy: boolean; valid: boolean; onSubmit: () => void; onClose: () => void; children: React.ReactNode; submitLabel: string }) {
  return (
    <Dialog open onOpenChange={(openState) => !openState && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader><DialogTitle>{title}</DialogTitle><DialogDescription>{description}</DialogDescription></DialogHeader>
        <form className="grid gap-4" onSubmit={(event) => { event.preventDefault(); if (valid && !busy) onSubmit(); }}>
          {children}
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>Cancel</Button>
            <Button type="submit" disabled={!valid || busy}>{busy && <Loader2 className="h-4 w-4 animate-spin" />}{submitLabel}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function AddExpenseDialog({ session, categories, accounts, methods, currency, onClose, onSaved, onError }: DialogActions & { categories: ExpenseCategoryDto[]; accounts: CashAccountDto[]; methods: PaymentMethodDto[]; currency: string }) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const [expenseDate, setExpenseDate] = React.useState(localIso());
  const [categoryId, setCategoryId] = React.useState<number | null>(null);
  const [amountMinor, setAmountMinor] = React.useState(0);
  const [methodId, setMethodId] = React.useState<number | null>(null);
  const [accountId, setAccountId] = React.useState<number | null>(null);
  const [note, setNote] = React.useState("");
  const [reference, setReference] = React.useState("");
  const [attachmentPath, setAttachmentPath] = React.useState("");
  const [newCategoryOpen, setNewCategoryOpen] = React.useState(false);
  const requestKey = React.useRef(idempotencyKey());

  const mutation = useMutation({
    mutationFn: () => expensePost(session, {
      expenseDate,
      categoryId: categoryId!,
      amountMinor,
      paymentMethodId: methodId!,
      cashAccountId: accountId!,
      description: note.trim(),
      reference: reference.trim() || null,
      attachmentPath: attachmentPath || null,
      payee: null,
      idempotencyKey: requestKey.current,
    }),
    onSuccess: onSaved,
    onError: (error: Error) => onError(error, "Could not add expense"),
  });
  const activeCategories = categories.filter((category) => category.isActive);
  const activeAccounts = accounts.filter((account) => account.isActive);
  const activeMethods = methods.filter((method) => method.isActive);
  const selectedAccount = activeAccounts.find((account) => account.id === accountId);
  const valid = !!expenseDate && categoryId !== null && methodId !== null && accountId !== null && amountMinor > 0 && (!selectedAccount || amountMinor <= selectedAccount.balanceMinor);

  const chooseAttachment = async () => {
    const selected = await open({ multiple: false, directory: false, title: "Choose bill attachment" });
    if (typeof selected === "string") setAttachmentPath(selected);
  };

  return (
    <>
      <ExpenseFormDialog title="Add Expense" description="The expense and matching account outflow are posted together." busy={mutation.isPending} valid={valid} onSubmit={() => mutation.mutate()} onClose={onClose} submitLabel="Add Expense">
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="grid gap-1.5"><Label htmlFor="expense-date">Date</Label><Input id="expense-date" type="date" value={expenseDate} onChange={(event) => setExpenseDate(event.target.value)} /></div>
          <div className="grid gap-1.5"><Label>Amount ({currency})</Label><MoneyInput value={amountMinor} onCommit={(value) => setAmountMinor(Math.max(0, value))} placeholder="0.00" /></div>
        </div>
        <div className="grid gap-1.5">
          <div className="flex items-center justify-between"><Label>Category</Label><Button type="button" variant="ghost" size="sm" onClick={() => setNewCategoryOpen(true)}><Plus className="h-3.5 w-3.5" /> New category</Button></div>
          <Select value={categoryId === null ? "" : String(categoryId)} onValueChange={(value) => setCategoryId(Number(value))}><SelectTrigger><SelectValue placeholder="Select a category" /></SelectTrigger><SelectContent>{activeCategories.map((category) => <SelectItem key={category.id} value={String(category.id)}>{category.name}</SelectItem>)}</SelectContent></Select>
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="grid gap-1.5"><Label>Payment method</Label><Select value={methodId === null ? "" : String(methodId)} onValueChange={(value) => setMethodId(Number(value))}><SelectTrigger><SelectValue placeholder="Select method" /></SelectTrigger><SelectContent>{activeMethods.map((method) => <SelectItem key={method.id} value={String(method.id)}>{method.name}</SelectItem>)}</SelectContent></Select></div>
          <div className="grid gap-1.5"><Label>Cash / bank account</Label><Select value={accountId === null ? "" : String(accountId)} onValueChange={(value) => setAccountId(Number(value))}><SelectTrigger><SelectValue placeholder="Select account" /></SelectTrigger><SelectContent>{activeAccounts.map((account) => <SelectItem key={account.id} value={String(account.id)}>{account.name} · {formatCurrency(account.balanceMinor, currency)}</SelectItem>)}</SelectContent></Select></div>
        </div>
        {selectedAccount && amountMinor > selectedAccount.balanceMinor && <p className="text-xs text-rose-600">Amount exceeds the available account balance.</p>}
        <div className="grid gap-1.5"><Label htmlFor="expense-note">Note (optional)</Label><Input id="expense-note" value={note} onChange={(event) => setNote(event.target.value)} placeholder="Electricity bill for September" /></div>
        <div className="grid gap-1.5"><Label htmlFor="expense-reference">Bill / reference number (optional)</Label><Input id="expense-reference" value={reference} onChange={(event) => setReference(event.target.value)} placeholder="BILL-00125" /></div>
        <div className="grid gap-1.5"><Label>Bill attachment (optional)</Label><div className="flex gap-2"><Input value={attachmentPath} readOnly placeholder="No file selected" /><Button type="button" variant="outline" onClick={() => void chooseAttachment()}><FileUp className="h-4 w-4" /> Browse</Button></div></div>
      </ExpenseFormDialog>
      {newCategoryOpen && <NewCategoryDialog session={session} onClose={() => setNewCategoryOpen(false)} onCreated={(created) => { setCategoryId(created.id); setNewCategoryOpen(false); void queryClient.invalidateQueries({ queryKey: ["expenses", "categories"] }); toast({ variant: "success", title: "Category added" }); }} onError={onError} />}
    </>
  );
}

function NewCategoryDialog({ session, onClose, onCreated, onError }: { session: string; onClose: () => void; onCreated: (category: ExpenseCategoryDto) => void; onError: (error: Error, title?: string) => void }) {
  const [name, setName] = React.useState("");
  const mutation = useMutation({ mutationFn: () => expenseCategoryCreate(session, { name: name.trim(), code: categoryCode(name), isActive: true }), onSuccess: onCreated, onError: (error: Error) => onError(error, "Could not add category") });
  return (
    <Dialog open onOpenChange={(openState) => !openState && onClose()}>
      <DialogContent className="sm:max-w-md"><DialogHeader><DialogTitle>New expense category</DialogTitle><DialogDescription>The new category will be selected without clearing the expense form.</DialogDescription></DialogHeader><form className="grid gap-4" onSubmit={(event) => { event.preventDefault(); if (name.trim() && !mutation.isPending) mutation.mutate(); }}><div className="grid gap-1.5"><Label htmlFor="new-category-name">Category name</Label><Input id="new-category-name" autoFocus value={name} onChange={(event) => setName(event.target.value)} placeholder="Office Supplies" /></div><DialogFooter><Button type="button" variant="ghost" onClick={onClose}>Cancel</Button><Button type="submit" disabled={!name.trim() || mutation.isPending}>{mutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />}Add category</Button></DialogFooter></form></DialogContent>
    </Dialog>
  );
}

function CategoryManagerDialog({ session, categories, loading, onClose, onChanged, onError }: { session: string; categories: ExpenseCategoryDto[]; loading: boolean; onClose: () => void; onChanged: () => void; onError: (error: Error, title?: string) => void }) {
  const [name, setName] = React.useState("");
  const [editing, setEditing] = React.useState<ExpenseCategoryDto | null>(null);
  const [editName, setEditName] = React.useState("");
  const createMutation = useMutation({ mutationFn: () => expenseCategoryCreate(session, { name: name.trim(), code: categoryCode(name), isActive: true }), onSuccess: () => { setName(""); onChanged(); }, onError: (error: Error) => onError(error, "Could not add category") });
  const updateMutation = useMutation({ mutationFn: (input: { category: ExpenseCategoryDto; name: string; active: boolean }) => expenseCategoryUpdate(session, { id: input.category.id, name: input.name.trim(), isActive: input.active }), onSuccess: () => { setEditing(null); onChanged(); }, onError: (error: Error) => onError(error, "Could not update category") });
  return (
    <Dialog open onOpenChange={(openState) => !openState && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader><DialogTitle>Manage Categories</DialogTitle><DialogDescription>Add, rename, or archive expense categories. Archived categories remain on historical expenses.</DialogDescription></DialogHeader>
        <form className="flex gap-2" onSubmit={(event) => { event.preventDefault(); if (name.trim() && !createMutation.isPending) createMutation.mutate(); }}><Input value={name} onChange={(event) => setName(event.target.value)} placeholder="e.g. Electricity" aria-label="New category name" /><Button type="submit" disabled={!name.trim() || createMutation.isPending}>{createMutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />}Add</Button></form>
        {loading ? <StatePanel loading message="Loading categories…" /> : (
          <div className="overflow-hidden rounded-lg border border-neutral-200">
            <Table><TableHeader><TableRow><TableHead>Name</TableHead><TableHead>Status</TableHead><TableHead className="text-right">Actions</TableHead></TableRow></TableHeader><TableBody>{categories.map((category) => <TableRow key={category.id}><TableCell><div className="font-medium">{category.name}</div><div className="text-xs text-neutral-400">{category.code}</div></TableCell><TableCell><Badge variant={category.isActive ? "success" : "neutral"}>{category.isActive ? "Active" : "Archived"}</Badge></TableCell><TableCell className="text-right"><Button variant="ghost" size="sm" onClick={() => { setEditing(category); setEditName(category.name); }}><Pencil className="h-3.5 w-3.5" /> Rename</Button><Button variant="ghost" size="sm" disabled={updateMutation.isPending} onClick={() => updateMutation.mutate({ category, name: category.name, active: !category.isActive })}>{category.isActive ? "Archive" : "Restore"}</Button></TableCell></TableRow>)}</TableBody></Table>
          </div>
        )}
        <DialogFooter><Button variant="outline" onClick={onClose}>Done</Button></DialogFooter>
        {editing && <Dialog open onOpenChange={(openState) => !openState && setEditing(null)}><DialogContent className="sm:max-w-md"><DialogHeader><DialogTitle>Rename category</DialogTitle><DialogDescription>Existing expenses stay linked to this category.</DialogDescription></DialogHeader><form className="grid gap-4" onSubmit={(event) => { event.preventDefault(); if (editName.trim() && !updateMutation.isPending) updateMutation.mutate({ category: editing, name: editName, active: editing.isActive }); }}><div className="grid gap-1.5"><Label htmlFor="rename-category">Name</Label><Input id="rename-category" autoFocus value={editName} onChange={(event) => setEditName(event.target.value)} /></div><DialogFooter><Button type="button" variant="ghost" onClick={() => setEditing(null)}>Cancel</Button><Button type="submit" disabled={!editName.trim() || updateMutation.isPending}>Save</Button></DialogFooter></form></DialogContent></Dialog>}
      </DialogContent>
    </Dialog>
  );
}

function ReverseExpenseDialog({ session, expense, currency, onClose, onSaved, onError }: DialogActions & { expense: ExpenseDto; currency: string }) {
  const [reason, setReason] = React.useState("");
  const mutation = useMutation({ mutationFn: () => expenseReverse(session, { expenseId: expense.id, reason: reason.trim() }), onSuccess: onSaved, onError: (error: Error) => onError(error, "Could not reverse expense") });
  return <ExpenseFormDialog title="Reverse expense" description={`Reverse ${formatCurrency(expense.amountMinor, currency)} and restore it to ${expense.cashAccountName}. The original entry remains in history.`} busy={mutation.isPending} valid={!!reason.trim()} onSubmit={() => mutation.mutate()} onClose={onClose} submitLabel="Reverse expense"><div className="grid gap-1.5"><Label htmlFor="reversal-reason">Reason</Label><Input id="reversal-reason" autoFocus value={reason} onChange={(event) => setReason(event.target.value)} placeholder="Why is this expense being reversed?" /></div></ExpenseFormDialog>;
}

function OwnerTransfers({ session, accounts, currency, onError }: { session: string; accounts: CashAccountDto[]; currency: string; onError: (error: Error, title?: string) => void }) {
  const queryClient = useQueryClient();
  const [openState, setOpenState] = React.useState(false);
  const [kind, setKind] = React.useState<"capital_in" | "withdrawal">("capital_in");
  const [amount, setAmount] = React.useState(0);
  const [date, setDate] = React.useState(localIso());
  const [accountId, setAccountId] = React.useState<number | null>(null);
  const [notes, setNotes] = React.useState("");
  const query = useQuery({ queryKey: ["expenses", "owner"], queryFn: () => ownerTransactionList(session, 100), enabled: !!session });
  const mutation = useMutation({ mutationFn: () => ownerTransactionPost(session, { kind, amountMinor: amount, transactionDate: date, cashAccountId: accountId!, notes: notes.trim() || null, idempotencyKey: `owner-${Date.now()}` }), onSuccess: () => { setOpenState(false); void query.refetch(); void queryClient.invalidateQueries({ queryKey: ["expenses", "accounts"] }); }, onError: (error: Error) => onError(error, "Could not record transfer") });
  return <div className="mt-4"><div className="flex justify-end"><Button size="sm" onClick={() => setOpenState(true)}><Plus className="h-4 w-4" /> Record transfer</Button></div>{query.isLoading ? <StatePanel loading message="Loading owner transfers…" /> : !query.data?.length ? <StatePanel message="No owner transfers recorded." /> : <div className="mt-3 overflow-hidden rounded-lg border"><Table><TableHeader><TableRow><TableHead>Date</TableHead><TableHead>Type</TableHead><TableHead>Account</TableHead><TableHead>Notes</TableHead><TableHead className="text-right">Amount</TableHead></TableRow></TableHeader><TableBody>{query.data.map((row) => <TableRow key={row.id}><TableCell>{readableDate(row.transactionDate)}</TableCell><TableCell>{row.kind === "capital_in" ? "Capital in" : "Withdrawal"}</TableCell><TableCell>{row.cashAccountName}</TableCell><TableCell>{row.notes || "—"}</TableCell><TableCell className="text-right font-medium">{formatCurrency(row.amountMinor, currency)}</TableCell></TableRow>)}</TableBody></Table></div>}{openState && <ExpenseFormDialog title="Owner transfer" description="Owner capital and withdrawals affect cash, not operational profit." busy={mutation.isPending} valid={amount > 0 && accountId !== null} onSubmit={() => mutation.mutate()} onClose={() => setOpenState(false)} submitLabel="Record transfer"><div className="grid gap-3 sm:grid-cols-2"><div className="grid gap-1.5"><Label>Type</Label><Select value={kind} onValueChange={(value) => setKind(value as typeof kind)}><SelectTrigger><SelectValue /></SelectTrigger><SelectContent><SelectItem value="capital_in">Capital in</SelectItem><SelectItem value="withdrawal">Withdrawal</SelectItem></SelectContent></Select></div><div className="grid gap-1.5"><Label>Date</Label><Input type="date" value={date} onChange={(event) => setDate(event.target.value)} /></div></div><div className="grid gap-1.5"><Label>Amount ({currency})</Label><MoneyInput value={amount} onCommit={(value) => setAmount(Math.max(0, value))} /></div><div className="grid gap-1.5"><Label>Account</Label><Select value={accountId === null ? "" : String(accountId)} onValueChange={(value) => setAccountId(Number(value))}><SelectTrigger><SelectValue placeholder="Select account" /></SelectTrigger><SelectContent>{accounts.filter((account) => account.isActive).map((account) => <SelectItem key={account.id} value={String(account.id)}>{account.name}</SelectItem>)}</SelectContent></Select></div><div className="grid gap-1.5"><Label>Notes (optional)</Label><Input value={notes} onChange={(event) => setNotes(event.target.value)} /></div></ExpenseFormDialog>}</div>;
}

function ProfitSummary({ session, currency }: { session: string; currency: string }) {
  const [from, setFrom] = React.useState("");
  const [to, setTo] = React.useState("");
  const query = useQuery({ queryKey: ["expenses", "profit", from, to], queryFn: () => profitSummary(session, from || null, to || null), enabled: !!session });
  return <div className="mt-4"><div className="flex flex-wrap gap-2"><Input className="w-44" type="date" value={from} onChange={(event) => setFrom(event.target.value)} /><Input className="w-44" type="date" value={to} onChange={(event) => setTo(event.target.value)} /></div>{query.isLoading ? <StatePanel loading message="Loading profit summary…" /> : query.data ? <ProfitCards data={query.data} currency={currency} /> : <StatePanel message="Profit summary is unavailable." />}</div>;
}

function ProfitCards({ data, currency }: { data: ProfitSummaryDto; currency: string }) {
  const rows = [["Revenue", data.revenueMinor], ["Cost of goods sold", -data.cogsMinor], ["Expenses", -data.expensesMinor], ["Operational profit", data.operationalProfitMinor], ["Net cash flow", data.netCashFlowMinor]] as const;
  return <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-5">{rows.map(([label, value]) => <div key={label} className="rounded-lg border border-neutral-200 p-3"><div className="text-xs text-neutral-500">{label}</div><div className={cn("mt-1 font-semibold tabular-nums", value < 0 && "text-rose-600")}>{formatCurrency(value, currency)}</div></div>)}</div>;
}
