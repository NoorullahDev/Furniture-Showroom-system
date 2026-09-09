"use client";

import * as React from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Archive, Loader2, Plus, Pencil } from "lucide-react";

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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import { isSessionError, useSession } from "@/components/session/session-provider";
import {
  categoryArchive,
  categoryCreate,
  categoryUpdate,
  categoryList,
  productTypeArchive,
  productTypeCreate,
  productTypeList,
  productTypeUpdate,
  type CategoryDto,
  type ProductTypeDto,
} from "@/lib/tauri/api";
import { commandErrorMessage } from "@/lib/tauri/client";

type Tab = "categories" | "types";

export function CategoryManagerDialog({
  onClose,
  onChanged,
}: {
  onClose: () => void;
  onChanged: () => void;
}) {
  const { toast } = useToast();
  const { refresh, profile } = useSession();
  const session = profile?.sessionId ?? "";
  const [tab, setTab] = React.useState<Tab>("categories");

  const categoriesQuery = useQuery({
    queryKey: ["catalogue", "categories"],
    queryFn: () => categoryList(session),
    enabled: !!session,
  });
  const typesQuery = useQuery({
    queryKey: ["catalogue", "types", "all"],
    queryFn: () => productTypeList(session, null),
    enabled: !!session,
  });

  const categories = categoriesQuery.data ?? [];
  const types = typesQuery.data ?? [];

  const invalidate = () => {
    void categoriesQuery.refetch();
    void typesQuery.refetch();
  };

  const handleError = (e: unknown, title: string) => {
    if (isSessionError(e)) {
      refresh();
      return;
    }
    toast({ variant: "error", title, description: commandErrorMessage(e) });
  };

  const [creatingCategory, setCreatingCategory] = React.useState(false);
  const [editingCategory, setEditingCategory] = React.useState<CategoryDto | null>(null);
  const [creatingType, setCreatingType] = React.useState(false);
  const [editingType, setEditingType] = React.useState<ProductTypeDto | null>(null);

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Organize catalogue</DialogTitle>
          <DialogDescription>
            Categories group products; product types refine a category (e.g. &quot;Chairs&quot;,
            &quot;Sofas&quot;).
          </DialogDescription>
        </DialogHeader>

        <div className="flex gap-1 border-b border-neutral-200">
          <TabButton active={tab === "categories"} onClick={() => setTab("categories")}>
            Categories ({categories.length})
          </TabButton>
          <TabButton active={tab === "types"} onClick={() => setTab("types")}>
            Product types ({types.length})
          </TabButton>
        </div>

        {tab === "categories" ? (
          <div className="max-h-[50vh] overflow-y-auto pr-1">
            <div className="mb-2 flex justify-end">
              <Button size="sm" onClick={() => setCreatingCategory(true)}>
                <Plus className="h-4 w-4" />
                Add category
              </Button>
            </div>
            <ul className="divide-y divide-neutral-100 rounded-md border border-neutral-200">
              {categories.length === 0 && (
                <li className="p-4 text-center text-sm text-neutral-500">No categories yet.</li>
              )}
              {categories.map((c) => (
                <li key={c.id} className="flex items-center gap-3 px-3 py-2.5">
                  <div className="min-w-0 flex-1">
                    <p className="flex items-center gap-2 text-sm font-medium text-neutral-900">
                      {c.name}
                      {!c.isActive && <Badge variant="neutral">Inactive</Badge>}
                    </p>
                    <p className="text-xs text-neutral-500">
                      {c.parentId != null ? "Subcategory" : "Root"} · {c.productCount}{" "}
                      product{c.productCount === 1 ? "" : "s"} · order {c.sortOrder}
                    </p>
                  </div>
                  <Button variant="ghost" size="icon" onClick={() => setEditingCategory(c)}>
                    <Pencil className="h-4 w-4" />
                  </Button>
                  <ArchiveCategoryButton
                    category={c}
                    onError={(e) => handleError(e, "Archive failed")}
                    onSaved={() => invalidate()}
                  />
                </li>
              ))}
            </ul>
          </div>
        ) : (
          <TypeTab
            categories={categories}
            types={types}
            onError={(e) => handleError(e, "Product type change failed")}
            onChanged={() => typesQuery.refetch()}
            onCreate={() => setCreatingType(true)}
            onEdit={setEditingType}
          />
        )}

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>

      {creatingCategory && (
        <CategoryFormDialog
          onClose={() => setCreatingCategory(false)}
          onSaved={() => {
            invalidate();
            onChanged();
          }}
          categories={categories}
        />
      )}
      {editingCategory && (
        <CategoryFormDialog
          category={editingCategory}
          onClose={() => setEditingCategory(null)}
          onSaved={() => {
            invalidate();
            onChanged();
          }}
          categories={categories}
        />
      )}
      {creatingType && (
        <ProductTypeFormDialog
          categories={categories}
          onClose={() => setCreatingType(false)}
          onSaved={() => {
            typesQuery.refetch();
            onChanged();
          }}
        />
      )}
      {editingType && (
        <ProductTypeFormDialog
          productType={editingType}
          categories={categories}
          onClose={() => setEditingType(null)}
          onSaved={() => {
            typesQuery.refetch();
            onChanged();
          }}
        />
      )}
    </Dialog>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={
        active
          ? "-mb-px border-b-2 border-forest-600 px-3 pb-2 text-sm font-medium text-forest-700"
          : "px-3 pb-2 text-sm text-neutral-500 hover:text-neutral-800"
      }
    >
      {children}
    </button>
  );
}

function ArchiveCategoryButton({
  category,
  onError,
  onSaved,
}: {
  category: CategoryDto;
  onError: (e: unknown) => void;
  onSaved: () => void;
}) {
  const { toast } = useToast();
  const { profile } = useSession();
  const session = profile?.sessionId ?? "";
  const mutation = useMutation({
    mutationFn: () => categoryArchive(session, category.id, "Archived from Organize dialog"),
    onSuccess: () => {
      toast({ variant: "success", title: `Category “${category.name}” archived` });
      onSaved();
    },
    onError: (e: unknown) => {
      if (!isSessionError(e)) onError(e);
    },
  });
  return (
    <Button
      variant="ghost"
      size="icon"
      disabled={mutation.isPending}
      onClick={() => {
        if (window.confirm(`Archive category “${category.name}”?`)) mutation.mutate();
      }}
    >
      {mutation.isPending ? (
        <Loader2 className="h-4 w-4 animate-spin" />
      ) : (
        <Archive className="h-4 w-4" />
      )}
    </Button>
  );
}

function TypeTab({
  categories,
  types,
  onError,
  onChanged,
  onCreate,
  onEdit,
}: {
  categories: CategoryDto[];
  types: ProductTypeDto[];
  onError: (e: unknown) => void;
  onChanged: () => void;
  onCreate: () => void;
  onEdit: (t: ProductTypeDto) => void;
}) {
  const { toast } = useToast();
  const { profile } = useSession();
  const session = profile?.sessionId ?? "";
  const catName = (id: number) => categories.find((c) => c.id === id)?.name ?? "?";

  const archiveMutation = useMutation({
    mutationFn: (t: ProductTypeDto) =>
      productTypeArchive(session, t.id, "Archived from Organize dialog"),
    onSuccess: () => {
      toast({ variant: "success", title: "Product type archived" });
      onChanged();
    },
    onError: (e: unknown) => {
      if (!isSessionError(e)) onError(e);
    },
  });

  return (
    <div className="max-h-[50vh] overflow-y-auto pr-1">
      <div className="mb-2 flex justify-end">
        <Button size="sm" onClick={onCreate} disabled={categories.length === 0}>
          <Plus className="h-4 w-4" />
          Add product type
        </Button>
      </div>
      <ul className="divide-y divide-neutral-100 rounded-md border border-neutral-200">
        {types.length === 0 && (
          <li className="p-4 text-center text-sm text-neutral-500">
            {categories.length === 0
              ? "Create a category first."
              : "No product types yet."}
          </li>
        )}
        {types.map((t) => (
          <li key={t.id} className="flex items-center gap-3 px-3 py-2.5">
            <div className="min-w-0 flex-1">
              <p className="flex items-center gap-2 text-sm font-medium text-neutral-900">
                {t.name}
                {!t.isActive && <Badge variant="neutral">Inactive</Badge>}
              </p>
              <p className="text-xs text-neutral-500">
                {catName(t.categoryId)} · {t.productCount} product{t.productCount === 1 ? "" : "s"}
              </p>
            </div>
            <Button variant="ghost" size="icon" onClick={() => onEdit(t)}>
              <Pencil className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              disabled={archiveMutation.isPending}
              onClick={() => {
                if (window.confirm(`Archive product type “${t.name}”?`)) archiveMutation.mutate(t);
              }}
            >
              <Archive className="h-4 w-4" />
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function CategoryFormDialog({
  category,
  categories,
  onClose,
  onSaved,
}: {
  category?: CategoryDto;
  categories: CategoryDto[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const { toast } = useToast();
  const { refresh, profile } = useSession();
  const session = profile?.sessionId ?? "";
  const isEdit = category !== undefined;

  const [name, setName] = React.useState(category?.name ?? "");
  const [parentId, setParentId] = React.useState<number | null>(category?.parentId ?? null);
  const [sortOrder, setSortOrder] = React.useState(category ? String(category.sortOrder) : "0");
  const [isActive, setIsActive] = React.useState(category?.isActive ?? true);

  const parents = categories.filter((c) => c.id !== category?.id);
  const mutation = useMutation({
    mutationFn: () =>
      isEdit
        ? categoryUpdate(session, {
            categoryId: category.id,
            name: name || null,
            parentId: parentId !== null ? parentId : undefined,
            sortOrder: Number(sortOrder || 0),
            isActive,
          })
        : categoryCreate(session, {
            name,
            parentId,
            sortOrder: Number(sortOrder || 0),
          }),
    onSuccess: () => {
      toast({ variant: "success", title: isEdit ? "Category updated" : "Category created" });
      onSaved();
      onClose();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Save failed", description: commandErrorMessage(e) });
    },
  });

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      toast({ variant: "error", title: "Save failed", description: "Name is required." });
      return;
    }
    mutation.mutate();
  }

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{isEdit ? "Edit category" : "Add category"}</DialogTitle>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid gap-1.5">
            <Label htmlFor="cat-name">Name *</Label>
            <Input id="cat-name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </div>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div className="grid gap-1.5">
              <Label>Parent</Label>
              <Select
                value={parentId !== null ? String(parentId) : "none"}
                onValueChange={(v) => setParentId(v === "none" ? null : Number(v))}
                disabled={parents.length === 0}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Root" />
                </SelectTrigger>
                <SelectContent>
                  {(!isEdit || category.parentId === null) && (
                    <SelectItem value="none">Root (no parent)</SelectItem>
                  )}
                  {parents.map((c) => (
                    <SelectItem key={c.id} value={String(c.id)}>
                      {c.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="cat-order">Sort order</Label>
              <Input
                id="cat-order"
                inputMode="numeric"
                value={sortOrder}
                onChange={(e) => setSortOrder(e.target.value.replace(/[^0-9-]/g, ""))}
              />
            </div>
          </div>
          {isEdit && (
            <div className="grid gap-1.5">
              <Label>Status</Label>
              <select
                value={isActive ? "active" : "inactive"}
                onChange={(e) => setIsActive(e.target.value === "active")}
                className="h-9 rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-forest-500/60"
              >
                <option value="active">Active</option>
                <option value="inactive">Inactive</option>
              </select>
            </div>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={mutation.isPending}>
              Cancel
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
              {isEdit ? "Save" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ProductTypeFormDialog({
  productType,
  categories,
  onClose,
  onSaved,
}: {
  productType?: ProductTypeDto;
  categories: CategoryDto[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const { toast } = useToast();
  const { refresh, profile } = useSession();
  const session = profile?.sessionId ?? "";
  const isEdit = productType !== undefined;

  const [categoryId, setCategoryId] = React.useState(productType?.categoryId ?? null);
  const [name, setName] = React.useState(productType?.name ?? "");
  const [isActive, setIsActive] = React.useState(productType?.isActive ?? true);

  const mutation = useMutation({
    mutationFn: () =>
      isEdit
        ? productTypeUpdate(session, {
            productTypeId: productType.id,
            name: name || null,
            isActive,
          })
        : productTypeCreate(session, categoryId as number, name),
    onSuccess: () => {
      toast({ variant: "success", title: isEdit ? "Product type updated" : "Product type created" });
      onSaved();
      onClose();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Save failed", description: commandErrorMessage(e) });
    },
  });

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      toast({ variant: "error", title: "Save failed", description: "Name is required." });
      return;
    }
    if (!isEdit && categoryId === null) {
      toast({ variant: "error", title: "Save failed", description: "Choose a category." });
      return;
    }
    mutation.mutate();
  }

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{isEdit ? "Edit product type" : "Add product type"}</DialogTitle>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid gap-1.5">
            <Label>Category</Label>
            <Select
              value={categoryId !== null ? String(categoryId) : "none"}
              onValueChange={(v) => setCategoryId(v === "none" ? null : Number(v))}
              disabled={isEdit}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select category" />
              </SelectTrigger>
              <SelectContent>
                {!isEdit && <SelectItem value="none">—</SelectItem>}
                {categories.map((c) => (
                  <SelectItem key={c.id} value={String(c.id)}>
                    {c.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="pt-name">Name *</Label>
            <Input id="pt-name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </div>
          {isEdit && (
            <div className="grid gap-1.5">
              <Label>Status</Label>
              <select
                value={isActive ? "active" : "inactive"}
                onChange={(e) => setIsActive(e.target.value === "active")}
                className="h-9 rounded-md border border-neutral-300 bg-white px-3 text-sm text-neutral-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-forest-500/60"
              >
                <option value="active">Active</option>
                <option value="inactive">Inactive</option>
              </select>
            </div>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={mutation.isPending}>
              Cancel
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
              {isEdit ? "Save" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}