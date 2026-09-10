"use client";

import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Banknote,
  CornerDownLeft,
  Loader2,
  PackageCheck,
  PackageOpen,
  Pin,
  Search,
  ShoppingCart,
  Truck,
  UserPlus,
  Warehouse,
  Wallet,
} from "lucide-react";

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
import { useToast } from "@/components/ui/toast";
import { isSessionError, useSession } from "@/components/session/session-provider";
import {
  cashAccountList,
  customerCreate,
  customerList,
  customerReceiptCreate,
  expenseCategoryList,
  expensePost,
  paymentMethodList,
  supplierCreate,
} from "@/lib/tauri/api";
import { commandErrorMessage } from "@/lib/tauri/client";
import { cn } from "@/lib/utils";
import type { ShellView } from "@/lib/shell";

const MAX_PINNED = 6;
const MAX_RECENT = 8;

type ShortForm = "customer" | "supplier" | "expense" | "receipt" | null;

type QuickAction = {
  id: string;
  label: string;
  description: string;
  icon: React.ElementType;
  permissionAny?: string[];
  form?: Exclude<ShortForm, null>;
  view?: ShellView;
};

const ACTIONS: QuickAction[] = [
  {
    id: "new-sale",
    label: "New sale",
    description: "Open the point-of-sale",
    icon: ShoppingCart,
    permissionAny: ["sale.create"],
    view: "sales",
  },
  {
    id: "add-product",
    label: "Add product",
    description: "Create a catalogue product",
    icon: PackageOpen,
    permissionAny: ["product.create"],
    view: "catalogue",
  },
  {
    id: "receive-payment",
    label: "Receive customer payment",
    description: "Record a receipt from a customer",
    icon: Banknote,
    permissionAny: ["payment.receive"],
    form: "receipt",
  },
  {
    id: "record-purchase",
    label: "Record purchase",
    description: "Create and post a supplier invoice",
    icon: Truck,
    permissionAny: ["purchase.create"],
    view: "purchases",
  },
  {
    id: "add-expense",
    label: "Add expense",
    description: "Post an expense from a cash account",
    icon: Wallet,
    permissionAny: ["expense.create"],
    form: "expense",
  },
  {
    id: "schedule-delivery",
    label: "Schedule delivery",
    description: "Plan a delivery for a confirmed sale",
    icon: PackageCheck,
    permissionAny: ["delivery.create"],
    view: "fulfilment",
  },
  {
    id: "add-customer",
    label: "Add customer",
    description: "Create a customer profile",
    icon: UserPlus,
    permissionAny: ["customer.create"],
    form: "customer",
  },
  {
    id: "add-supplier",
    label: "Add supplier",
    description: "Create a supplier profile",
    icon: Warehouse,
    permissionAny: ["supplier.create"],
    form: "supplier",
  },
];

function todayLocal(): string {
  const d = new Date();
  const m = `${d.getMonth() + 1}`.padStart(2, "0");
  const day = `${d.getDate()}`.padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

function readIds(key: string): string[] {
  try {
    const raw = localStorage.getItem(key);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? parsed.filter((x): x is string => typeof x === "string")
      : [];
  } catch {
    return [];
  }
}

function writeIds(key: string, ids: string[]) {
  try {
    localStorage.setItem(key, JSON.stringify(ids));
  } catch {
    // Pinned/recent persistence is best-effort.
  }
}

function FormShell({
  title,
  description,
  onBack,
  onSubmit,
  submitLabel,
  busy,
  error,
  children,
}: {
  title: string;
  description: string;
  onBack: () => void;
  onSubmit: (e: React.FormEvent) => void;
  submitLabel: string;
  busy: boolean;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <form onSubmit={onSubmit} className="grid gap-4 p-6">
      <DialogHeader>
        <button
          type="button"
          onClick={onBack}
          className="mb-1 w-fit text-xs font-medium text-forest-700 hover:underline"
        >
          Back to commands
        </button>
        <DialogTitle className="text-base">{title}</DialogTitle>
        <DialogDescription>{description}</DialogDescription>
      </DialogHeader>
      <div className="grid gap-4">{children}</div>
      {error ? <p className="text-xs text-red-600">{error}</p> : null}
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onBack} disabled={busy}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy}>
          {busy ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : null}
          {submitLabel}
        </Button>
      </DialogFooter>
    </form>
  );
}

function CustomerForm({ onDone, onBack }: { onDone: () => void; onBack: () => void }) {
  const { profile, refresh } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const [code, setCode] = React.useState("");
  const [name, setName] = React.useState("");
  const [phone, setPhone] = React.useState("");
  const [email, setEmail] = React.useState("");
  const [creditDays, setCreditDays] = React.useState("30");
  const [err, setErr] = React.useState("");
  const [busy, setBusy] = React.useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr("");
    if (!code.trim() || !name.trim()) {
      setErr("Code and name are required.");
      return;
    }
    setBusy(true);
    try {
      await customerCreate(session, {
        code: code.trim(),
        name: name.trim(),
        phone: phone.trim() || null,
        email: email.trim() || null,
        address: null,
        creditLimitMinor: 0,
        creditDays: Number(creditDays) || 0,
        openingBalanceMinor: 0,
        isActive: true,
      });
      void queryClient.invalidateQueries({ queryKey: ["selling"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      toast({ variant: "success", title: "Customer added" });
      onDone();
    } catch (e2) {
      if (isSessionError(e2)) {
        refresh();
        return;
      }
      setErr(commandErrorMessage(e2) || "Could not add the customer.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <FormShell
      title="Add customer"
      description="A short profile so you can sell on credit and track the ledger."
      onBack={onBack}
      onSubmit={submit}
      submitLabel="Add customer"
      busy={busy}
      error={err}
    >
      <div className="grid gap-1.5">
        <Label htmlFor="qa-cust-code">Code</Label>
        <Input
          id="qa-cust-code"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          placeholder="e.g. CUST-0021"
        />
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="qa-cust-name">Name</Label>
        <Input
          id="qa-cust-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Customer name"
        />
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="qa-cust-phone">Phone</Label>
          <Input
            id="qa-cust-phone"
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            placeholder="Optional"
          />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="qa-cust-email">Email</Label>
          <Input
            id="qa-cust-email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="Optional"
          />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="qa-cust-credit-days">Credit terms (days)</Label>
        <Input
          id="qa-cust-credit-days"
          inputMode="numeric"
          value={creditDays}
          onChange={(e) => setCreditDays(e.target.value)}
        />
      </div>
    </FormShell>
  );
}

function SupplierForm({ onDone, onBack }: { onDone: () => void; onBack: () => void }) {
  const { profile, refresh } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const [code, setCode] = React.useState("");
  const [name, setName] = React.useState("");
  const [phone, setPhone] = React.useState("");
  const [email, setEmail] = React.useState("");
  const [address, setAddress] = React.useState("");
  const [err, setErr] = React.useState("");
  const [busy, setBusy] = React.useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr("");
    if (!code.trim() || !name.trim()) {
      setErr("Code and name are required.");
      return;
    }
    setBusy(true);
    try {
      await supplierCreate(session, {
        code: code.trim(),
        name: name.trim(),
        phone: phone.trim() || null,
        email: email.trim() || null,
        address: address.trim() || null,
        openingBalanceMinor: 0,
        isActive: true,
      });
      void queryClient.invalidateQueries({ queryKey: ["purchasing"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      toast({ variant: "success", title: "Supplier added" });
      onDone();
    } catch (e2) {
      if (isSessionError(e2)) {
        refresh();
        return;
      }
      setErr(commandErrorMessage(e2) || "Could not add the supplier.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <FormShell
      title="Add supplier"
      description="A short profile so you can record purchases and payables."
      onBack={onBack}
      onSubmit={submit}
      submitLabel="Add supplier"
      busy={busy}
      error={err}
    >
      <div className="grid gap-1.5">
        <Label htmlFor="qa-supp-code">Code</Label>
        <Input
          id="qa-supp-code"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          placeholder="e.g. SUPP-0008"
        />
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="qa-supp-name">Name</Label>
        <Input
          id="qa-supp-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Supplier name"
        />
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="qa-supp-phone">Phone</Label>
          <Input
            id="qa-supp-phone"
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            placeholder="Optional"
          />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="qa-supp-email">Email</Label>
          <Input
            id="qa-supp-email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="Optional"
          />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="qa-supp-address">Address</Label>
        <Input
          id="qa-supp-address"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          placeholder="Optional"
        />
      </div>
    </FormShell>
  );
}

function ExpenseForm({
  onDone,
  onBack,
}: {
  onDone: () => void;
  onBack: () => void;
}) {
  const { profile, refresh } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const [categoryId, setCategoryId] = React.useState("");
  const [accountId, setAccountId] = React.useState("");
  const [amount, setAmount] = React.useState(0);
  const [date, setDate] = React.useState(todayLocal);
  const [description, setDescription] = React.useState("");
  const [payee, setPayee] = React.useState("");
  const [err, setErr] = React.useState("");
  const [busy, setBusy] = React.useState(false);

  const categoriesQuery = useQuery({
    queryKey: ["qa", "expense-categories"],
    queryFn: () => expenseCategoryList(session),
    enabled: !!session,
  });
  const accountsQuery = useQuery({
    queryKey: ["qa", "cash-accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session,
  });

  const categories = categoriesQuery.data ?? [];
  const accounts = accountsQuery.data ?? [];

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr("");
    if (!categoryId) {
      setErr("Choose an expense category.");
      return;
    }
    if (!accountId) {
      setErr("Choose the cash account that paid.");
      return;
    }
    if (amount <= 0) {
      setErr("Enter an amount greater than zero.");
      return;
    }
    if (!description.trim()) {
      setErr("Enter a short description.");
      return;
    }
    setBusy(true);
    try {
      await expensePost(session, {
        categoryId: Number(categoryId),
        amountMinor: amount,
        expenseDate: date,
        cashAccountId: Number(accountId),
        description: description.trim(),
        payee: payee.trim() || null,
        reference: null,
        idempotencyKey: null,
      });
      void queryClient.invalidateQueries({ queryKey: ["expenses"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      toast({ variant: "success", title: "Expense posted" });
      onDone();
    } catch (e2) {
      if (isSessionError(e2)) {
        refresh();
        return;
      }
      setErr(commandErrorMessage(e2) || "Could not post the expense.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <FormShell
      title="Add expense"
      description="Post a cash expense and it moves straight into the cash book."
      onBack={onBack}
      onSubmit={submit}
      submitLabel="Post expense"
      busy={busy}
      error={err}
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="qa-exp-cat">Category</Label>
          <Select value={categoryId} onValueChange={setCategoryId}>
            <SelectTrigger id="qa-exp-cat" className="w-full">
              <SelectValue placeholder="Select category" />
            </SelectTrigger>
            <SelectContent>
              {categories.map((c) => (
                <SelectItem key={c.id} value={String(c.id)}>
                  {c.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="qa-exp-account">Cash account</Label>
          <Select value={accountId} onValueChange={setAccountId}>
            <SelectTrigger id="qa-exp-account" className="w-full">
              <SelectValue placeholder="Select cash account" />
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
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="qa-exp-amount">Amount</Label>
          <MoneyInput id="qa-exp-amount" value={amount} onCommit={setAmount} />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="qa-exp-date">Date</Label>
          <Input id="qa-exp-date" type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="qa-exp-desc">Description</Label>
        <Input
          id="qa-exp-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="e.g. Electricity bill"
        />
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="qa-exp-payee">Payee</Label>
        <Input
          id="qa-exp-payee"
          value={payee}
          onChange={(e) => setPayee(e.target.value)}
          placeholder="Optional"
        />
      </div>
    </FormShell>
  );
}

function ReceiptForm({
  onDone,
  onBack,
}: {
  onDone: () => void;
  onBack: () => void;
}) {
  const { profile, refresh } = useSession();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const [customerId, setCustomerId] = React.useState("");
  const [methodId, setMethodId] = React.useState("");
  const [accountId, setAccountId] = React.useState("");
  const [amount, setAmount] = React.useState(0);
  const [date, setDate] = React.useState(todayLocal);
  const [err, setErr] = React.useState("");
  const [busy, setBusy] = React.useState(false);

  const customersQuery = useQuery({
    queryKey: ["qa", "customers"],
    queryFn: () => customerList(session),
    enabled: !!session,
  });
  const methodsQuery = useQuery({
    queryKey: ["qa", "payment-methods"],
    queryFn: () => paymentMethodList(session),
    enabled: !!session,
  });
  const accountsQuery = useQuery({
    queryKey: ["qa", "cash-accounts"],
    queryFn: () => cashAccountList(session),
    enabled: !!session,
  });

  const customers = customersQuery.data ?? [];
  const methods = methodsQuery.data ?? [];
  const accounts = accountsQuery.data ?? [];

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr("");
    if (!customerId) {
      setErr("Choose the customer.");
      return;
    }
    if (!methodId) {
      setErr("Choose a payment method.");
      return;
    }
    if (!accountId) {
      setErr("Choose the cash account that receives the money.");
      return;
    }
    if (amount <= 0) {
      setErr("Enter an amount greater than zero.");
      return;
    }
    setBusy(true);
    try {
      await customerReceiptCreate(session, {
        customerId: Number(customerId),
        paymentMethodId: Number(methodId),
        cashAccountId: Number(accountId),
        paymentDate: date,
        amountMinor: amount,
        notes: null,
        idempotencyKey: null,
        allocations: null,
      });
      void queryClient.invalidateQueries({ queryKey: ["selling"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      toast({ variant: "success", title: "Payment received" });
      onDone();
    } catch (e2) {
      if (isSessionError(e2)) {
        refresh();
        return;
      }
      setErr(commandErrorMessage(e2) || "Could not record the payment.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <FormShell
      title="Receive customer payment"
      description="Record a receipt and allocate it against the oldest open invoice."
      onBack={onBack}
      onSubmit={submit}
      submitLabel="Receive payment"
      busy={busy}
      error={err}
    >
      <div className="grid gap-1.5">
        <Label htmlFor="qa-receipt-customer">Customer</Label>
        <Select value={customerId} onValueChange={setCustomerId}>
          <SelectTrigger id="qa-receipt-customer" className="w-full">
            <SelectValue placeholder="Select customer" />
          </SelectTrigger>
          <SelectContent>
            {customers.map((c) => (
              <SelectItem key={c.id} value={String(c.id)}>
                {c.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="qa-receipt-amount">Amount</Label>
          <MoneyInput id="qa-receipt-amount" value={amount} onCommit={setAmount} />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="qa-receipt-date">Date</Label>
          <Input
            id="qa-receipt-date"
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
        </div>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="qa-receipt-method">Payment method</Label>
          <Select value={methodId} onValueChange={setMethodId}>
            <SelectTrigger id="qa-receipt-method" className="w-full">
              <SelectValue placeholder="Select method" />
            </SelectTrigger>
            <SelectContent>
              {methods.map((m) => (
                <SelectItem key={m.id} value={String(m.id)}>
                  {m.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="qa-receipt-account">Cash account</Label>
          <Select value={accountId} onValueChange={setAccountId}>
            <SelectTrigger id="qa-receipt-account" className="w-full">
              <SelectValue placeholder="Select cash account" />
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
      </div>
    </FormShell>
  );
}

export function QuickAddPalette({
  open,
  onOpenChange,
  onNavigate,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onNavigate: (view: ShellView) => void;
}) {
  const { profile, hasPermission } = useSession();
  const { toast } = useToast();
  const userId = profile?.userId ?? 0;

  const pinKey = React.useMemo(() => `qa:pinned:${userId}`, [userId]);
  const recentKey = React.useMemo(() => `qa:recent:${userId}`, [userId]);

  const [query, setQuery] = React.useState("");
  const [activeIndex, setActiveIndex] = React.useState(0);
  const [form, setForm] = React.useState<ShortForm>(null);
  const [pinned, setPinned] = React.useState<string[]>([]);
  const [recent, setRecent] = React.useState<string[]>([]);
  const [loaded, setLoaded] = React.useState(false);

  React.useEffect(() => {
    if (open) {
      setQuery("");
      setActiveIndex(0);
      setForm(null);
      setPinned(readIds(pinKey));
      setRecent(readIds(recentKey));
      setLoaded(false);
      const raf = requestAnimationFrame(() => setLoaded(true));
      return () => cancelAnimationFrame(raf);
    }
    setLoaded(false);
  }, [open, pinKey, recentKey]);

  React.useEffect(() => {
    if (loaded) writeIds(pinKey, pinned);
  }, [pinKey, pinned, loaded]);
  React.useEffect(() => {
    if (loaded) writeIds(recentKey, recent);
  }, [recentKey, recent, loaded]);

  const canViewCustomers = hasPermission("customer.view");

  const visible = React.useMemo(() => {
    const q = query.trim().toLowerCase();
    return ACTIONS.filter((a) => {
      const allowed = !a.permissionAny || a.permissionAny.some((p) => hasPermission(p));
      if (!allowed) return false;
      if (!q) return true;
      return `${a.label} ${a.description}`.toLowerCase().includes(q);
    });
  }, [query, hasPermission]);

  const ordered = React.useMemo(() => {
    const map = new Map(ACTIONS.map((a) => [a.id, a]));
    const seen = new Set<string>();
    const out: { action: QuickAction; section: string }[] = [];
    const isVisible = new Set(visible.map((a) => a.id));
    const push = (ids: string[], section: string) => {
      for (const id of ids) {
        if (seen.has(id)) continue;
        const action = map.get(id);
        if (action && isVisible.has(id)) {
          out.push({ action, section });
          seen.add(id);
        }
      }
    };
    if (!query.trim()) {
      push(pinned, "Pinned");
      push(recent, "Recent");
    }
    for (const a of visible) {
      if (!seen.has(a.id)) out.push({ action: a, section: query.trim() ? "Matches" : "Actions" });
    }
    return out;
  }, [visible, pinned, recent, query]);

  React.useEffect(() => {
    setActiveIndex((i) => Math.min(i, Math.max(0, ordered.length - 1)));
  }, [ordered.length]);

  const pushRecent = (id: string) => {
    setRecent((prev) => [id, ...prev.filter((x) => x !== id)].slice(0, MAX_RECENT));
  };

  const done = () => {
    setForm(null);
    onOpenChange(false);
  };

  const run = (a: QuickAction) => {
    pushRecent(a.id);
    if (a.form === "receipt" && !canViewCustomers) {
      onNavigate("sales");
      onOpenChange(false);
      return;
    }
    if (a.view) {
      onNavigate(a.view);
      onOpenChange(false);
      return;
    }
    if (a.form) {
      setForm(a.form);
    }
  };

  const togglePin = (id: string) => {
    if (pinned.includes(id)) {
      setPinned((p) => p.filter((x) => x !== id));
      return;
    }
    if (pinned.length >= MAX_PINNED) {
      toast({
        variant: "error",
        title: "Pin limit reached",
        description: `You can pin up to ${MAX_PINNED} actions.`,
      });
      return;
    }
    setPinned((p) => [...p, id]);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, ordered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = ordered[activeIndex];
      if (item) run(item.action);
    }
  };

  const renderForm = () => {
    const back = () => setForm(null);
    switch (form) {
      case "customer":
        return <CustomerForm onDone={done} onBack={back} />;
      case "supplier":
        return <SupplierForm onDone={done} onBack={back} />;
      case "expense":
        return <ExpenseForm onDone={done} onBack={back} />;
      case "receipt":
        return <ReceiptForm onDone={done} onBack={back} />;
      default:
        return null;
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-w-xl gap-0 overflow-hidden p-0"
        onKeyDown={form ? undefined : onKeyDown}
      >
        {form ? (
          renderForm()
        ) : (
          <div className="flex flex-col">
            <div className="flex items-center gap-2.5 border-b border-neutral-100 px-4 py-3">
              <Search className="h-4 w-4 shrink-0 text-neutral-400" />
              <Input
                autoFocus
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setActiveIndex(0);
                }}
                placeholder="Type a command…"
                className="border-0 px-0 shadow-none focus-visible:ring-0 focus-visible:border-transparent"
              />
            </div>
            <div className="max-h-[380px] overflow-y-auto py-2">
              {ordered.length === 0 ? (
                <p className="px-4 py-8 text-center text-sm text-neutral-400">
                  No matching command
                </p>
              ) : (
                ordered.map((item, idx) => {
                  const showHeader = idx === 0 || item.section !== ordered[idx - 1].section;
                  const isPinned = pinned.includes(item.action.id);
                  return (
                    <div key={item.action.id}>
                      {showHeader && (
                        <p className="px-4 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-neutral-400">
                          {item.section}
                        </p>
                      )}
                      <div
                        role="option"
                        aria-selected={idx === activeIndex}
                        className={cn(
                          "mx-2 flex items-center gap-3 rounded-md px-2 py-2",
                          idx === activeIndex && "bg-forest-50",
                        )}
                      >
                        <button
                          type="button"
                          onClick={() => run(item.action)}
                          onMouseEnter={() => setActiveIndex(idx)}
                          className="flex min-w-0 flex-1 items-center gap-3 rounded-md text-left"
                        >
                          <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-forest-100 text-forest-700">
                            <item.action.icon className="h-4 w-4" />
                          </span>
                          <span className="min-w-0">
                            <span className="block truncate text-sm font-medium text-neutral-800">
                              {item.action.label}
                            </span>
                            <span className="block truncate text-xs text-neutral-500">
                              {item.action.description}
                            </span>
                          </span>
                        </button>
                        <button
                          type="button"
                          onClick={() => togglePin(item.action.id)}
                          aria-pressed={isPinned}
                          aria-label={isPinned ? "Unpin action" : "Pin action"}
                          className={cn(
                            "shrink-0 rounded p-1.5 transition-colors",
                            isPinned
                              ? "text-amber-600 hover:text-amber-700"
                              : "text-neutral-300 hover:text-neutral-500",
                          )}
                        >
                          <Pin className="h-4 w-4" />
                        </button>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-neutral-100 px-4 py-2.5 text-[11px] text-neutral-400">
              <span className="flex items-center gap-1">
                <CornerDownLeft className="h-3 w-3" /> to select
              </span>
              <span>
                <kbd className="font-sans">↑</kbd>/<kbd className="font-sans">↓</kbd> to navigate
              </span>
              <span>Esc to close</span>
              <span className="ml-auto uppercase">Ctrl+K for data search</span>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}