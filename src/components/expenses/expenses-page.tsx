"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowDownCircle,
  ArrowUpCircle,
  Loader2,
  Plus,
  ReceiptText,
  RectangleEllipsis,
  Tags,
  Undo2,
  Wallet,
} from "lucide-react";

import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { MoneyInput } from "@/components/ui/money-input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
import { useToast } from "@/components/ui/toast";
import { isSessionError, useSession } from "@/components/session/session-provider";
import {
  cashAccountList,
  expenseCategoryCreate,
  expenseCategoryList,
  expenseCategoryUpdate,
  expenseList,
  expensePost,
  expenseReverse,
  ownerTransactionList,
  ownerTransactionPost,
  profitSummary,
  type CashAccountDto,
  type ExpenseCategoryDto,
  type ExpenseDto,
  type ProfitSummaryDto,
} from "@/lib/tauri/api";
import { formatPkr } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";

type Tab = "expenses" | "categories" | "owner" | "profit";

function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export function ExpensesPage() {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const canView = hasPermission("expense.view");
  const canCreate = hasPermission("expense.create");
  const canReverse = hasPermission("expense.reverse");
  const canProfit = hasPermission("profit.view");
  const canOwner = hasPermission("owner.transfer");

  const [view, setView] = React.useState<Tab>("expenses");
  const [dialog, setDialog] = React.useState<null | "post" | "category" | "owner">(null);
  const [reverseTarget, setReverseTarget] = React.useState<ExpenseDto | null>(null);
  const [profitFrom, setProfitFrom] = React.useState<string>("");
  const [profitTo, setProfitTo] = React.useState<string>("");
  const [accountFilter, setAccountFilter] = React.useState<number | null>(null);

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["expenses"] });
    void queryClient.invalidateQueries({ queryKey: ["purchasing"] });
  };

  const accountsQuery = useQuery({
    queryKey: ["expenses", "accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session && canView,
  });
  const categoriesQuery = useQuery({
    queryKey: ["expenses", "categories"],
    queryFn: () => expenseCategoryList(session),
    enabled: !!session && canView,
  });
  const expensesQuery = useQuery({
    queryKey: ["expenses", "list", accountFilter],
    queryFn: () =>
      expenseList(session, {
        status: null,
        categoryId: null,
        cashAccountId: accountFilter,
        fromDate: null,
        toDate: null,
        limit: 200,
        offset: null,
      }),
    enabled: !!session && canView,
  });
  const ownerQuery = useQuery({
    queryKey: ["expenses", "owner"],
    queryFn: () => ownerTransactionList(session, 100),
    enabled: !!session && canOwner && view === "owner",
  });
  const profitQuery = useQuery({
    queryKey: ["expenses", "profit", profitFrom, profitTo],
    queryFn: () => profitSummary(session, profitFrom || null, profitTo || null),
    enabled: !!session && canProfit && view === "profit",
  });

  const accounts = accountsQuery.data ?? [];
  const categories = categoriesQuery.data ?? [];
  const expenses = expensesQuery.data ?? [];
  const ownerTransactions = ownerQuery.data ?? [];

  const posted = expenses.filter((e) => e.status === "posted");
  const totalExpenses = posted.reduce((acc, e) => acc + e.amountMinor, 0);
  const cashOnHand = accounts.reduce((acc, a) => acc + a.balanceMinor, 0);

  const done = (message: string) => () => {
    invalidate();
    setDialog(null);
    setReverseTarget(null);
    toast({ variant: "success", title: message });
  };
  const failed = (e: Error) => {
    if (isSessionError(e)) {
      refresh();
      return;
    }
    toast({ variant: "error", title: "Operation failed", description: commandErrorMessage(e) });
  };

  return (
    <div>
      <PageHeader
        title="Finance"
        subtitle="Expenses, cash accounts, owner transfers, and profit."
        actions={
          <div className="flex flex-wrap items-center gap-2">
            {canView && (
              <Select
                value={accountFilter ? String(accountFilter) : "all"}
                onValueChange={(v) => setAccountFilter(v === "all" ? null : Number(v))}
              >
                <SelectTrigger className="w-44">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All cash accounts</SelectItem>
                  {accounts.map((a) => (
                    <SelectItem key={a.id} value={String(a.id)}>
                      {a.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {canCreate && (
              <Button onClick={() => setDialog("post")}>
                <Plus className="h-4 w-4" />
                Post expense
              </Button>
            )}
          </div>
        }
      />

      <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-4">
        <SummaryCard label="Expenses (posted)" value={totalExpenses} money accent="rose" />
        <SummaryCard label="Cash on hand" value={cashOnHand} money accent="gold" />
        <SummaryCard
          label="Gross profit"
          value={profitQuery.data?.grossProfitMinor ?? 0}
          money
          accent={(profitQuery.data?.grossProfitMinor ?? 0) >= 0 ? "forest" : "rose"}
        />
        <SummaryCard
          label="Operational profit"
          value={profitQuery.data?.operationalProfitMinor ?? 0}
          money
          accent={(profitQuery.data?.operationalProfitMinor ?? 0) >= 0 ? "green" : "rose"}
        />
      </div>

      <div className="mt-5 flex items-center gap-1 overflow-x-auto border-b border-neutral-200">
        <TabButton active={view === "expenses"} onClick={() => setView("expenses")} icon={<ReceiptText className="h-4 w-4" />}>
          Expenses
        </TabButton>
        <TabButton active={view === "categories"} onClick={() => setView("categories")} icon={<Tags className="h-4 w-4" />}>
          Categories
        </TabButton>
        {canOwner && (
          <TabButton active={view === "owner"} onClick={() => setView("owner")} icon={<Wallet className="h-4 w-4" />}>
            Owner transfers
          </TabButton>
        )}
        {canProfit && (
          <TabButton active={view === "profit"} onClick={() => setView("profit")} icon={<RectangleEllipsis className="h-4 w-4" />}>
            Profit
          </TabButton>
        )}
      </div>

      <div className="mt-5">
        {view === "expenses" && (
          <ExpensesTable
            rows={expenses}
            loading={expensesQuery.isLoading}
            canReverse={canReverse}
            onReverse={setReverseTarget}
          />
        )}
        {view === "categories" && (
          <CategoriesTable
            rows={categories}
            loading={categoriesQuery.isLoading}
            canCreate={canCreate}
            onCreate={() => setDialog("category")}
          />
        )}
        {view === "owner" && canOwner && (
          <OwnerTable rows={ownerTransactions} loading={ownerQuery.isLoading} canCreate={canOwner} onCreate={() => setDialog("owner")} />
        )}
        {view === "profit" && canProfit && (
          <ProfitView
            summary={profitQuery.data}
            loading={profitQuery.isLoading}
            from={profitFrom}
            to={profitTo}
            onFrom={setProfitFrom}
            onTo={setProfitTo}
          />
        )}
      </div>

      {dialog === "post" && canCreate && (
        <PostExpenseDialog
          session={session}
          categories={categories}
          accounts={accounts}
          onClose={() => setDialog(null)}
          onDone={done("Expense posted")}
          onError={failed}
        />
      )}
      {dialog === "category" && canCreate && (
        <CategoryDialog
          session={session}
          onClose={() => setDialog(null)}
          onDone={() => done("Category saved")()}
          onError={failed}
        />
      )}
      {dialog === "owner" && canOwner && (
        <OwnerDialog
          session={session}
          accounts={accounts}
          onClose={() => setDialog(null)}
          onDone={() => done("Transfer recorded")()}
          onError={failed}
        />
      )}
      {reverseTarget && canReverse && (
        <ReverseExpenseDialog
          session={session}
          expense={reverseTarget}
          onClose={() => setReverseTarget(null)}
          onDone={done("Expense reversed")}
          onError={failed}
        />
      )}
    </div>
  );
}

function SummaryCard({
  label,
  value,
  money,
  accent = "neutral",
}: {
  label: string;
  value: number;
  money?: boolean;
  accent?: "gold" | "rose" | "green" | "forest" | "neutral";
}) {
  return (
    <div className="rounded-lg border border-neutral-200 bg-white p-3 shadow-sm">
      <p className="text-[11px] font-medium uppercase tracking-wide text-neutral-500">{label}</p>
      <p
        className={cn(
          "mt-1 text-xl font-semibold tabular-nums",
          accent === "gold" && "text-amber-600",
          accent === "rose" && "text-rose-600",
          accent === "green" && "text-emerald-600",
          accent === "forest" && "text-forest-700",
        )}
      >
        {money ? formatPkr(value) : value.toLocaleString()}
      </p>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  icon,
  children,
}: {
  active: boolean;
  onClick: () => void;
  icon?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 whitespace-nowrap border-b-2 px-3 py-2 text-sm font-medium transition-colors",
        active
          ? "border-forest-600 text-forest-700"
          : "border-transparent text-neutral-500 hover:text-neutral-800",
      )}
    >
      {icon}
      {children}
    </button>
  );
}

function LoadingRow() {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-neutral-200 bg-white p-10 text-sm text-neutral-500">
      <Loader2 className="h-4 w-4 animate-spin" />
      Loading…
    </div>
  );
}

function EmptyRow({ message }: { message: string }) {
  return (
    <div className="rounded-lg border border-dashed border-neutral-300 bg-white p-10 text-center text-sm text-neutral-500">
      {message}
    </div>
  );
}

function ExpenseStatusBadge({ status }: { status: string }) {
  if (status === "posted") return <Badge variant="success">Posted</Badge>;
  if (status === "reversed") return <Badge variant="warning">Reversed</Badge>;
  return <Badge variant="neutral">Draft</Badge>;
}

function ExpensesTable({
  rows,
  loading,
  canReverse,
  onReverse,
}: {
  rows: ExpenseDto[];
  loading: boolean;
  canReverse: boolean;
  onReverse: (e: ExpenseDto) => void;
}) {
  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No expenses recorded yet." />;
  return (
    <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Number</TableHead>
              <TableHead>Date</TableHead>
              <TableHead>Category</TableHead>
              <TableHead>Description</TableHead>
              <TableHead>Account</TableHead>
              <TableHead className="text-right">Amount</TableHead>
              <TableHead className="text-right">Status</TableHead>
              {canReverse && <TableHead className="w-16" />}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((e) => (
              <TableRow key={e.id}>
                <TableCell className="font-medium text-neutral-900">
                  {e.expenseNumber ?? `#${e.id}`}
                </TableCell>
                <TableCell className="whitespace-nowrap text-neutral-500">{e.expenseDate}</TableCell>
                <TableCell>
                  <span className="font-medium text-neutral-700">{e.categoryName}</span>
                  <span className="block text-[11px] text-neutral-500">{e.categoryCode}</span>
                </TableCell>
                <TableCell className="max-w-64 text-neutral-600">
                  <span className="block truncate">{e.description}</span>
                  {e.payee && <span className="block text-[11px] text-neutral-500">to {e.payee}</span>}
                </TableCell>
                <TableCell className="text-neutral-600">{e.cashAccountName}</TableCell>
                <TableCell className="text-right font-semibold tabular-nums text-rose-600">
                  {formatPkr(e.amountMinor)}
                </TableCell>
                <TableCell className="text-right">
                  <ExpenseStatusBadge status={e.status} />
                </TableCell>
                {canReverse && (
                  <TableCell className="text-right">
                    {e.status === "posted" && (
                      <Button
                        variant="ghost"
                        size="icon"
                        title="Reverse this expense"
                        onClick={() => onReverse(e)}
                      >
                        <Undo2 className="h-3.5 w-3.5 text-neutral-500" />
                      </Button>
                    )}
                  </TableCell>
                )}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function CategoriesTable({
  rows,
  loading,
  canCreate,
  onCreate,
}: {
  rows: ExpenseCategoryDto[];
  loading: boolean;
  canCreate: boolean;
  onCreate: () => void;
}) {
  const queryClient = useQueryClient();
  const { toast } = useToast();
  const session = useSession().profile?.sessionId ?? "";
  const [editing, setEditing] = React.useState<ExpenseCategoryDto | null>(null);
  const [name, setName] = React.useState("");

  const saveMutation = useMutation({
    mutationFn: () =>
      expenseCategoryUpdate(session, { id: editing!.id, name: name.trim(), isActive: editing!.isActive }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["expenses", "categories"] });
      setEditing(null);
      toast({ variant: "success", title: "Category updated" });
    },
    onError: (e: Error) => {
      toast({ variant: "error", title: "Update failed", description: commandErrorMessage(e) });
    },
  });

  const toggle = (c: ExpenseCategoryDto) => {
    expenseCategoryUpdate(session, { id: c.id, name: c.name, isActive: !c.isActive })
      .then(() => {
        void queryClient.invalidateQueries({ queryKey: ["expenses", "categories"] });
      })
      .catch((e: Error) => {
        toast({ variant: "error", title: "Update failed", description: commandErrorMessage(e) });
      });
  };

  if (loading) return <LoadingRow />;
  if (rows.length === 0) return <EmptyRow message="No expense categories yet." />;

  return (
    <div className="grid gap-4">
      {canCreate && (
        <div className="flex justify-end">
          <Button variant="outline" onClick={onCreate}>
            <Plus className="h-4 w-4" />
            New category
          </Button>
        </div>
      )}
      <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Code</TableHead>
                <TableHead>Name</TableHead>
                <TableHead className="text-right">Active</TableHead>
                {canCreate && <TableHead className="w-24" />}
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((c) => (
                <TableRow key={c.id}>
                  <TableCell>
                    <span className="font-mono text-xs text-neutral-500">{c.code}</span>
                  </TableCell>
                  <TableCell className="font-medium text-neutral-900">{c.name}</TableCell>
                  <TableCell className="text-right">
                    <Badge variant={c.isActive ? "success" : "neutral"}>
                      {c.isActive ? "Active" : "Inactive"}
                    </Badge>
                  </TableCell>
                  {canCreate && (
                    <TableCell className="text-right">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          setEditing(c);
                          setName(c.name);
                        }}
                      >
                        Rename
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => toggle(c)}>
                        {c.isActive ? "Deactivate" : "Activate"}
                      </Button>
                    </TableCell>
                  )}
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      </div>

      {editing && (
        <Dialog open onOpenChange={(o) => !o && setEditing(null)}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Rename category {editing.code}</DialogTitle>
              <DialogDescription>Rename an expense category.</DialogDescription>
            </DialogHeader>
            <div className="grid gap-1.5">
              <Label>Name</Label>
              <Input value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <DialogFooter>
              <Button variant="ghost" onClick={() => setEditing(null)}>
                Cancel
              </Button>
              <Button disabled={!name.trim()} onClick={() => saveMutation.mutate()}>
                {saveMutation.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                Save
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}

function OwnerTable({
  rows,
  loading,
  canCreate,
  onCreate,
}: {
  rows: { transactionNumber: string; kind: string; amountMinor: number; transactionDate: string; cashAccountName: string; notes?: string | null }[];
  loading: boolean;
  canCreate: boolean;
  onCreate: () => void;
}) {
  if (loading) return <LoadingRow />;
  return (
    <div className="grid gap-4">
      {canCreate && (
        <div className="flex justify-end">
          <Button variant="outline" onClick={onCreate}>
            <Plus className="h-4 w-4" />
            Record transfer
          </Button>
        </div>
      )}
      {rows.length === 0 ? (
        <EmptyRow message="No owner transfers yet." />
      ) : (
        <div className="overflow-hidden rounded-lg border border-neutral-200 bg-white">
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Number</TableHead>
                  <TableHead>Date</TableHead>
                  <TableHead>Kind</TableHead>
                  <TableHead>Account</TableHead>
                  <TableHead>Notes</TableHead>
                  <TableHead className="text-right">Amount</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((r) => (
                  <TableRow key={r.transactionNumber}>
                    <TableCell className="font-medium text-neutral-900">{r.transactionNumber}</TableCell>
                    <TableCell className="whitespace-nowrap text-neutral-500">{r.transactionDate}</TableCell>
                    <TableCell>
                      <Badge variant={r.kind === "capital_in" ? "success" : "warning"}>
                        {r.kind === "capital_in" ? "Capital in" : "Withdrawal"}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-neutral-600">{r.cashAccountName}</TableCell>
                    <TableCell className="max-w-56 truncate text-neutral-500">{r.notes ?? "—"}</TableCell>
                    <TableCell
                      className={cn(
                        "text-right font-semibold tabular-nums",
                        r.kind === "capital_in" ? "text-emerald-700" : "text-rose-600",
                      )}
                    >
                      {r.kind === "capital_in" ? "+" : "−"}
                      {formatPkr(r.amountMinor)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </div>
      )}
    </div>
  );
}

function ProfitView({
  summary,
  loading,
  from,
  to,
  onFrom,
  onTo,
}: {
  summary: ProfitSummaryDto | undefined;
  loading: boolean;
  from: string;
  to: string;
  onFrom: (v: string) => void;
  onTo: (v: string) => void;
}) {
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-end gap-2">
        <div className="grid gap-1.5">
          <Label>From</Label>
          <Input type="date" value={from} onChange={(e) => onFrom(e.target.value)} />
        </div>
        <div className="grid gap-1.5">
          <Label>To</Label>
          <Input type="date" value={to} onChange={(e) => onTo(e.target.value)} />
        </div>
        {from || to ? (
          <Button variant="ghost" size="sm" onClick={() => {
            onFrom("");
            onTo("");
          }}>
            Clear range
          </Button>
        ) : null}
      </div>

      {loading ? (
        <LoadingRow />
      ) : !summary ? (
        <EmptyRow message="Open the Profit tab to see the summary." />
      ) : (
        <div className="grid gap-4 lg:grid-cols-2">
          <ProfitCard
            title="Operating profit (document dates)"
            description="Revenue, cost of goods sold and operational expenses on the document date; owner transfers and purchase/customer cash are excluded."
            rows={[
              { label: "Net revenue (net of returns)", value: summary.revenueMinor },
              { label: "Cost of goods sold", value: summary.cogsMinor },
              { label: "Gross profit", value: summary.grossProfitMinor, strong: true },
              { label: "Delivery income", value: summary.deliveryIncomeMinor, mute: true },
              { label: "Operational expenses", value: -summary.expensesMinor },
              { label: "Damage write-off loss", value: -summary.damageLossMinor },
              { label: "Operational profit", value: summary.operationalProfitMinor, strong: true },
            ]}
          />
          <ProfitCard
            title="Cash movement (payment dates)"
            description="Money that actually moved between cash accounts in the range, independent of document dates."
            rows={[
              { label: "Cash inflow", value: summary.cashInflowMinor },
              { label: "Cash outflow", value: -summary.cashOutflowMinor },
              { label: "Net cash flow", value: summary.netCashFlowMinor, strong: true },
              { label: "Owner capital in", value: summary.ownerCapitalInMinor, mute: true },
              { label: "Owner withdrawals", value: -summary.ownerWithdrawalsMinor, mute: true },
            ]}
          />
        </div>
      )}
    </div>
  );
}

function ProfitCard({
  title,
  description,
  rows,
}: {
  title: string;
  description: string;
  rows: { label: string; value: number; strong?: boolean; mute?: boolean }[];
}) {
  return (
    <div className="rounded-lg border border-neutral-200 bg-white p-4 shadow-sm">
      <p className="text-sm font-semibold text-neutral-900">{title}</p>
      <p className="mt-0.5 text-[11px] text-neutral-500">{description}</p>
      <div className="mt-3 divide-y divide-neutral-100">
        {rows.map((r) => (
          <div key={r.label} className="flex items-center justify-between py-1.5">
            <span className={cn("text-sm", r.mute ? "text-neutral-400" : "text-neutral-600")}>{r.label}</span>
            <span
              className={cn(
                "tabular-nums",
                r.strong ? "font-semibold text-neutral-900" : "text-neutral-700",
                r.value < 0 && "text-rose-600",
              )}
            >
              {formatPkr(r.value)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

type DialogProps = {
  session: string;
  onClose: () => void;
  onDone: () => void;
  onError: (e: Error) => void;
};

function FormDialog({
  title,
  description,
  children,
  onSubmit,
  busy,
  submitLabel = "Save",
  onClose,
  submitDisabled,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
  onSubmit: () => void;
  busy: boolean;
  submitLabel?: string;
  onClose: () => void;
  submitDisabled?: boolean;
}) {
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <form
          className="grid gap-4"
          onSubmit={(e) => {
            e.preventDefault();
            if (!busy && !submitDisabled) onSubmit();
          }}
        >
          {children}
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || submitDisabled}>
              {busy && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {busy ? "Working…" : submitLabel}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function PostExpenseDialog({
  session,
  categories,
  accounts,
  onClose,
  onDone,
  onError,
}: DialogProps & { categories: ExpenseCategoryDto[]; accounts: CashAccountDto[] }) {
  const [categoryId, setCategoryId] = React.useState<number | null>(null);
  const [amountMinor, setAmountMinor] = React.useState(0);
  const [expenseDate, setExpenseDate] = React.useState(todayIso());
  const [accountId, setAccountId] = React.useState<number | null>(null);
  const [description, setDescription] = React.useState("");
  const [payee, setPayee] = React.useState("");
  const [reference, setReference] = React.useState("");
  const [attachmentPath, setAttachmentPath] = React.useState("");

  const mutation = useMutation({
    mutationFn: () =>
      expensePost(session, {
        categoryId: categoryId!,
        amountMinor,
        expenseDate,
        cashAccountId: accountId!,
        description: description.trim(),
        payee: payee.trim() || null,
        reference: reference.trim() || null,
        attachmentPath: attachmentPath.trim() || null,
        idempotencyKey: `expense-${Date.now()}`,
      }),
    onSuccess: onDone,
    onError,
  });

  const activeCategories = categories.filter((c) => c.isActive);
  const valid = categoryId !== null && amountMinor > 0 && accountId !== null && description.trim().length > 0;

  return (
    <FormDialog
      title="Post expense"
      description="Records the expense, pays it from a cash account and writes the cash entry in one transaction."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Post expense"
      onClose={onClose}
      submitDisabled={!valid}
    >
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Date</Label>
          <Input type="date" value={expenseDate} onChange={(e) => setExpenseDate(e.target.value)} />
        </div>
        <div className="grid gap-1.5">
          <Label>Amount (PKR)</Label>
          <MoneyInput value={amountMinor} onCommit={(v) => setAmountMinor(v < 0 ? 0 : v)} placeholder="0.00" />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Category</Label>
        <Select value={categoryId ? String(categoryId) : ""} onValueChange={(v) => setCategoryId(Number(v))}>
          <SelectTrigger>
            <SelectValue placeholder="Select a category" />
          </SelectTrigger>
          <SelectContent>
            {activeCategories.map((c) => (
              <SelectItem key={c.id} value={String(c.id)}>
                {c.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="grid gap-1.5">
        <Label>Cash account</Label>
        <Select value={accountId ? String(accountId) : ""} onValueChange={(v) => setAccountId(Number(v))}>
          <SelectTrigger>
            <SelectValue placeholder="Select an account" />
          </SelectTrigger>
          <SelectContent>
            {accounts.map((a) => (
              <SelectItem key={a.id} value={String(a.id)}>
                {a.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="grid gap-1.5">
        <Label>Description</Label>
        <Input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Shop rent for June" />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Payee (optional)</Label>
          <Input value={payee} onChange={(e) => setPayee(e.target.value)} placeholder="Landlord" />
        </div>
        <div className="grid gap-1.5">
          <Label>Reference (optional)</Label>
          <Input value={reference} onChange={(e) => setReference(e.target.value)} placeholder="Invoice no." />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Attachment path (optional)</Label>
        <Input value={attachmentPath} onChange={(e) => setAttachmentPath(e.target.value)} placeholder="C:\docs\nrent.pdf" />
      </div>
    </FormDialog>
  );
}

function ReverseExpenseDialog({
  session,
  expense,
  onClose,
  onDone,
  onError,
}: DialogProps & { expense: ExpenseDto }) {
  const [reason, setReason] = React.useState("");

  const mutation = useMutation({
    mutationFn: () => expenseReverse(session, { expenseId: expense.id, reason: reason.trim() }),
    onSuccess: onDone,
    onError,
  });

  return (
    <FormDialog
      title="Reverse expense"
      description={`Refunds ${formatPkr(expense.amountMinor)} from ${expense.cashAccountName} and marks "${expense.expenseNumber ?? expense.description}" reversed.`}
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Reverse expense"
      onClose={onClose}
      submitDisabled={!reason.trim()}
    >
      <div className="grid gap-1.5">
        <Label>Reason</Label>
        <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Posted by mistake" />
      </div>
    </FormDialog>
  );
}

function CategoryDialog({ session, onClose, onDone, onError }: DialogProps) {
  const [code, setCode] = React.useState("");
  const [name, setName] = React.useState("");

  const mutation = useMutation({
    mutationFn: () =>
      expenseCategoryCreate(session, { code: code.trim(), name: name.trim(), isActive: true }),
    onSuccess: onDone,
    onError,
  });

  return (
    <FormDialog
      title="New expense category"
      description="Add a category for recording expenses."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Add category"
      onClose={onClose}
      submitDisabled={code.trim().length === 0 || name.trim().length === 0}
    >
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Code</Label>
          <Input value={code} onChange={(e) => setCode(e.target.value)} placeholder="RENT" />
        </div>
        <div className="grid gap-1.5">
          <Label>Name</Label>
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Rent" />
        </div>
      </div>
    </FormDialog>
  );
}

function OwnerDialog({
  session,
  accounts,
  onClose,
  onDone,
  onError,
}: DialogProps & { accounts: CashAccountDto[] }) {
  const [kind, setKind] = React.useState<"capital_in" | "withdrawal">("capital_in");
  const [amountMinor, setAmountMinor] = React.useState(0);
  const [transactionDate, setTransactionDate] = React.useState(todayIso());
  const [accountId, setAccountId] = React.useState<number | null>(null);
  const [notes, setNotes] = React.useState("");

  const mutation = useMutation({
    mutationFn: () =>
      ownerTransactionPost(session, {
        kind,
        amountMinor,
        transactionDate,
        cashAccountId: accountId!,
        notes: notes.trim() || null,
        idempotencyKey: `owner-${Date.now()}`,
      }),
    onSuccess: onDone,
    onError,
  });

  const selected = accounts.find((a) => a.id === accountId);
  const valid =
    amountMinor > 0 && accountId !== null && !(kind === "withdrawal" && selected && amountMinor > selected.balanceMinor);

  return (
    <FormDialog
      title="Owner transfer"
      description="Owner capital contributions and withdrawals move cash but never participate in operational profit."
      onSubmit={() => mutation.mutate()}
      busy={mutation.isPending}
      submitLabel="Record transfer"
      onClose={onClose}
      submitDisabled={!valid}
    >
      <div className="grid gap-1.5">
        <Label>Type</Label>
        <Select value={kind} onValueChange={(v) => setKind(v as "capital_in" | "withdrawal")}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="capital_in">
              <ArrowUpCircle className="mr-1 inline h-4 w-4" /> Capital in (owner invests)
            </SelectItem>
            <SelectItem value="withdrawal">
              <ArrowDownCircle className="mr-1 inline h-4 w-4" /> Withdrawal (owner takes)
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label>Date</Label>
          <Input type="date" value={transactionDate} onChange={(e) => setTransactionDate(e.target.value)} />
        </div>
        <div className="grid gap-1.5">
          <Label>Amount (PKR)</Label>
          <MoneyInput value={amountMinor} onCommit={(v) => setAmountMinor(v < 0 ? 0 : v)} placeholder="0.00" />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label>Cash account</Label>
        <Select value={accountId ? String(accountId) : ""} onValueChange={(v) => setAccountId(Number(v))}>
          <SelectTrigger>
            <SelectValue placeholder="Select an account" />
          </SelectTrigger>
          <SelectContent>
            {accounts.map((a) => (
              <SelectItem key={a.id} value={String(a.id)}>
                {a.name} ({formatPkr(a.balanceMinor)})
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {selected && (
        <p className="text-sm text-neutral-500">
          Balance: {formatPkr(selected.balanceMinor)}
          {kind === "withdrawal" && amountMinor > selected.balanceMinor && (
            <span className="ml-1 text-rose-600">— exceeds the account balance</span>
          )}
        </p>
      )}
      <div className="grid gap-1.5">
        <Label>Notes (optional)</Label>
        <Input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Initial capital" />
      </div>
    </FormDialog>
  );
}