"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Archive,
  ArchiveRestore,
  Copy,
  FolderTree,
  LayoutGrid,
  List,
  Loader2,
  PackageOpen,
  Pencil,
  Plus,
  Search,
} from "lucide-react";

import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
  categoryList,
  productArchive,
  productList,
  productTypeList,
  productUnarchive,
  bundleList,
  locationList,
  type ProductListItemDto,
  type BundleDto,
} from "@/lib/tauri/api";
import { formatPkr } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { takeDashboardTarget } from "@/lib/dashboard-navigation";
import { StoredImage } from "@/components/catalogue/stored-image";
import { ProductDetailDialog } from "@/components/catalogue/product-detail-dialog";
import { ProductEditorDialog } from "@/components/catalogue/product-editor-dialog";
import { CategoryManagerDialog } from "@/components/catalogue/category-manager-dialog";
import { DuplicateProductDialog } from "@/components/catalogue/duplicate-product-dialog";
import { SetsTable, BundleDialog, BundleAvailabilityDialog } from "@/components/sales/sales-page";
import { Layers, Package } from "lucide-react";

type Scope = "active" | "archived" | "all";

export function CataloguePage() {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const canMutate = hasPermission("product.create");
  const canViewCost = hasPermission("product.cost.view");
  const canManageBundles = hasPermission("bundle.create");
  const canViewBundles = hasPermission("bundle.view");
  const dashboardTarget = React.useMemo(() => takeDashboardTarget("catalogue"), []);

  const [q, setQ] = React.useState("");
  const [debouncedQ, setDebouncedQ] = React.useState("");
  const [scope, setScope] = React.useState<Scope>("active");
  const [categoryId, setCategoryId] = React.useState<number | null>(null);
  const [productTypeId, setProductTypeId] = React.useState<number | null>(null);
  const [priceMin, setPriceMin] = React.useState("");
  const [priceMax, setPriceMax] = React.useState("");
  const [attributeQ, setAttributeQ] = React.useState("");
  const [view, setView] = React.useState<"grid" | "table">("grid");
  const [viewTab, setViewTab] = React.useState<"products" | "sets">("products");

  const [detail, setDetail] = React.useState<ProductListItemDto | null>(null);
  const [editing, setEditing] = React.useState<ProductListItemDto | null>(null);
  const [creating, setCreating] = React.useState(
    dashboardTarget?.target === "new-product" && canMutate,
  );
  const [duplicating, setDuplicating] = React.useState<ProductListItemDto | null>(null);
  const [managing, setManaging] = React.useState(false);
  const [activeBundle, setActiveBundle] = React.useState<BundleDto | null>(null);
  const [dialog, setDialog] = React.useState<null | "bundle" | "bundle-detail">(null);

  React.useEffect(() => {
    const t = window.setTimeout(() => setDebouncedQ(q), 300);
    return () => window.clearTimeout(t);
  }, [q]);

  React.useEffect(() => {
    setProductTypeId(null);
  }, [categoryId]);

  const categoriesQuery = useQuery({
    queryKey: ["catalogue", "categories"],
    queryFn: () => categoryList(session),
    enabled: !!session,
  });
  const typesQuery = useQuery({
    queryKey: ["catalogue", "types", categoryId],
    queryFn: () => productTypeList(session, categoryId),
    enabled: !!session,
  });
  const productsQuery = useQuery({
    queryKey: [
      "catalogue",
      "products",
      scope,
      debouncedQ,
      categoryId,
      productTypeId,
      priceMin,
      priceMax,
      attributeQ,
    ],
    queryFn: () =>
      productList(session, {
        scope,
        q: debouncedQ || undefined,
        categoryId,
        productTypeId,
        priceMin: priceMin !== "" ? Number(priceMin) : null,
        priceMax: priceMax !== "" ? Number(priceMax) : null,
        attributeQ: attributeQ || null,
      }),
    enabled: !!session,
  });
  const bundlesQuery = useQuery({
    queryKey: ["selling", "bundles"],
    queryFn: () => bundleList(session),
    enabled: !!session && (canViewBundles || canManageBundles),
  });
  const locationsQuery = useQuery({
    queryKey: ["selling", "locations"],
    queryFn: () => locationList(session),
    enabled: !!session,
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["catalogue", "products"] });
    void queryClient.invalidateQueries({ queryKey: ["catalogue", "categories"] });
    void queryClient.invalidateQueries({ queryKey: ["selling", "bundles"] });
  };

  const archiveMutation = useMutation({
    mutationFn: (productId: number) =>
      productArchive(session, productId, `Archived from catalogue (${scope})`),
    onSuccess: () => {
      toast({ variant: "success", title: "Product archived" });
      invalidate();
      setDetail(null);
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Archive failed", description: commandErrorMessage(e) });
    },
  });

  const unarchiveMutation = useMutation({
    mutationFn: (productId: number) => productUnarchive(session, productId),
    onSuccess: () => {
      toast({ variant: "success", title: "Product restored" });
      invalidate();
      setDetail(null);
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Restore failed", description: commandErrorMessage(e) });
    },
  });

  const categories = categoriesQuery.data ?? [];
  const products = productsQuery.data ?? [];
  const types = typesQuery.data ?? [];
  const bundles = bundlesQuery.data ?? [];
  const locations = locationsQuery.data ?? [];

  return (
    <div>
      <PageHeader
        title="Catalogue"
        subtitle="Products, categories and product types."
        actions={
          <>
            <Button variant="outline" onClick={() => setManaging(true)}>
              <FolderTree className="h-4 w-4" />
              Organize
            </Button>
            {viewTab === "products" && canMutate && (
              <Button onClick={() => setCreating(true)}>
                <Plus className="h-4 w-4" />
                New product
              </Button>
            )}
            {viewTab === "sets" && canViewBundles && (
              <Button
                variant="outline"
                onClick={() => {
                  setActiveBundle(null);
                  setDialog("bundle-detail");
                }}
              >
                <Layers className="h-4 w-4" />
                Set availability
              </Button>
            )}
            {viewTab === "sets" && canManageBundles && (
              <Button
                onClick={() => {
                  setActiveBundle(null);
                  setDialog("bundle");
                }}
              >
                <Package className="h-4 w-4" />
                New set
              </Button>
            )}
          </>
        }
      />

      <div className="mt-5 flex items-center gap-1 overflow-x-auto border-b border-neutral-200">
        <button
          type="button"
          onClick={() => setViewTab("products")}
          className={`flex items-center gap-2 border-b-2 px-4 py-2.5 text-sm font-medium transition-colors ${
            viewTab === "products" ? "border-forest-600 text-forest-700" : "border-transparent text-neutral-500 hover:text-neutral-700"
          }`}
        >
          <Package className="h-4 w-4" />
          Products
        </button>
        <button
          type="button"
          onClick={() => setViewTab("sets")}
          className={`flex items-center gap-2 border-b-2 px-4 py-2.5 text-sm font-medium transition-colors ${
            viewTab === "sets" ? "border-forest-600 text-forest-700" : "border-transparent text-neutral-500 hover:text-neutral-700"
          }`}
        >
          <Layers className="h-4 w-4" />
          Furniture sets
        </button>
      </div>

      {viewTab === "products" && (
        <>
          <div className="mt-5 flex flex-wrap items-end gap-2">
        <div className="relative min-w-52 flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-400" />
          <Input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search article, name, category…"
            className="pl-8"
          />
        </div>
        <Select
          value={scope}
          onValueChange={(v) => setScope(v as Scope)}
        >
          <SelectTrigger className="w-32">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="active">Active</SelectItem>
            <SelectItem value="archived">Archived</SelectItem>
            <SelectItem value="all">All</SelectItem>
          </SelectContent>
        </Select>
        <Select value={categoryId ? String(categoryId) : "all"} onValueChange={(v) => setCategoryId(v === "all" ? null : Number(v))}>
          <SelectTrigger className="w-44">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All categories</SelectItem>
            {categories.map((c) => (
              <SelectItem key={c.id} value={String(c.id)}>
                {c.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select
          value={productTypeId ? String(productTypeId) : "all"}
          onValueChange={(v) => setProductTypeId(v === "all" ? null : Number(v))}
          disabled={!categoryId || types.length === 0}
        >
          <SelectTrigger className="w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All product types</SelectItem>
            {types.map((t) => (
              <SelectItem key={t.id} value={String(t.id)}>
                {t.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="grid gap-1">
          <Label className="text-[10px] uppercase tracking-wide text-neutral-400">Price (min – max)</Label>
          <div className="flex items-center gap-1">
            <Input
              value={priceMin}
              onChange={(e) => setPriceMin(e.target.value.replace(/[^0-9]/g, ""))}
              placeholder="Min"
              inputMode="numeric"
              className="w-20"
              aria-label="Minimum price"
            />
            <span className="text-neutral-400">–</span>
            <Input
              value={priceMax}
              onChange={(e) => setPriceMax(e.target.value.replace(/[^0-9]/g, ""))}
              placeholder="Max"
              inputMode="numeric"
              className="w-20"
              aria-label="Maximum price"
            />
          </div>
        </div>
        <div className="grid gap-1">
          <Label className="text-[10px] uppercase tracking-wide text-neutral-400">Attribute</Label>
          <Input
            value={attributeQ}
            onChange={(e) => setAttributeQ(e.target.value)}
            placeholder="e.g. oak"
            className="w-32"
          />
        </div>
        <Select disabled>
          <SelectTrigger className="w-36" title="Supplier filtering arrives with supplier master data">
            <SelectValue placeholder="Supplier (later)" />
          </SelectTrigger>
        </Select>
        <Select disabled>
          <SelectTrigger className="w-40" title="Stock-level filtering arrives in the inventory phase">
            <SelectValue placeholder="Stock status (Phase 4)" />
          </SelectTrigger>
        </Select>
        <div className="ml-auto flex rounded-md border border-neutral-200">
          <Button
            variant="ghost"
            size="sm"
            className={view === "grid" ? "bg-neutral-100" : ""}
            onClick={() => setView("grid")}
            aria-label="Grid view"
            aria-pressed={view === "grid"}
          >
            <LayoutGrid className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className={view === "table" ? "bg-neutral-100" : ""}
            onClick={() => setView("table")}
            aria-label="Table view"
            aria-pressed={view === "table"}
          >
            <List className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {productsQuery.isLoading && !products.length ? (
            <div className="mt-10 flex items-center justify-center gap-2 text-neutral-500">
              <Loader2 className="h-4 w-4 animate-spin" />
              Loading products…
            </div>
          ) : products.length === 0 ? (
            <div className="mt-10 flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-neutral-300 bg-white py-16 text-neutral-500">
              <PackageOpen className="h-8 w-8 text-neutral-300" />
              <p className="text-sm">No products found{debouncedQ ? " for this search" : ""}.</p>
            </div>
          ) : view === "table" ? (
            <ProductsTable
              products={products}
              canViewCost={canViewCost}
              canMutate={canMutate}
              onOpen={(p) => setDetail(p)}
              onEdit={(p) => {
                setEditing(p);
                setDetail(null);
              }}
              onDuplicate={(p) => {
                setDuplicating(p);
                setDetail(null);
              }}
              onArchive={(p) => {
                if (window.confirm(`Archive “${p.name}”?`)) archiveMutation.mutate(p.id);
              }}
              onUnarchive={(p) => unarchiveMutation.mutate(p.id)}
            />
          ) : (
            <div className="mt-5 grid grid-cols-2 gap-4 md:grid-cols-3 xl:grid-cols-4">
              {products.map((p) => (
                <ProductCard
                  key={p.id}
                  product={p}
                  canViewCost={canViewCost}
                  canMutate={canMutate}
                  onOpen={() => setDetail(p)}
                  onEdit={() => {
                    setEditing(p);
                    setDetail(null);
                  }}
                  onDuplicate={() => {
                    setDuplicating(p);
                    setDetail(null);
                  }}
                  onArchive={() => {
                    if (window.confirm(`Archive “${p.name}”?`)) archiveMutation.mutate(p.id);
                  }}
                  onUnarchive={() => unarchiveMutation.mutate(p.id)}
                />
              ))}
            </div>
          )}
        </>
      )}

      {viewTab === "sets" && (
        <SetsTable
          rows={bundles}
          loading={bundlesQuery.isLoading}
          canManage={canManageBundles}
          onEdit={(b) => {
            setActiveBundle(b);
            setDialog("bundle");
          }}
        />
      )}

      {detail && <ProductDetailDialog productId={detail.id} onClose={() => setDetail(null)} />}
      {creating && (
        <ProductEditorDialog onClose={() => setCreating(false)} onSaved={invalidate} />
      )}
      {editing && (
        <ProductEditorDialog
          productId={editing.id}
          onClose={() => setEditing(null)}
          onSaved={invalidate}
        />
      )}
      {duplicating && (
        <DuplicateProductDialog
          product={duplicating}
          onClose={() => setDuplicating(null)}
          onSaved={invalidate}
        />
      )}
      {managing && (
        <CategoryManagerDialog onClose={() => setManaging(false)} onChanged={invalidate} />
      )}
      {dialog === "bundle" && (
        <BundleDialog
          session={session}
          bundle={activeBundle}
          onClose={() => setDialog(null)}
          onDone={() => {
            invalidate();
            setDialog(null);
            setActiveBundle(null);
            toast({ variant: "success", title: activeBundle ? "Set updated" : "Set created" });
          }}
          onError={(e) => {
            if (isSessionError(e)) return refresh();
            toast({ variant: "error", title: "Operation failed", description: commandErrorMessage(e) });
          }}
        />
      )}
      {dialog === "bundle-detail" && (
        <BundleAvailabilityDialog
          session={session}
          bundles={bundles}
          locations={locations}
          onClose={() => setDialog(null)}
        />
      )}
    </div>
  );
}

function ProductCard({
  product,
  canViewCost,
  canMutate,
  onOpen,
  onEdit,
  onDuplicate,
  onArchive,
  onUnarchive,
}: {
  product: ProductListItemDto;
  canViewCost: boolean;
  canMutate: boolean;
  onOpen: () => void;
  onEdit: () => void;
  onDuplicate: () => void;
  onArchive: () => void;
  onUnarchive: () => void;
}) {
  const thumb = product.primaryThumbnailPath;

  return (
    <div className="group flex flex-col overflow-hidden rounded-lg border border-neutral-200 bg-white shadow-sm transition-shadow hover:shadow-md">
      <button
        type="button"
        onClick={onOpen}
        className="relative flex aspect-square items-center justify-center overflow-hidden bg-neutral-100"
      >
        <StoredImage
          path={thumb}
          className="h-full w-full object-cover"
        />
        <span className="absolute left-2 top-2">
          {product.archivedAt ? (
            <Badge variant="neutral">Archived</Badge>
          ) : product.isActive ? (
            <Badge variant="accent">Active</Badge>
          ) : (
            <Badge variant="warning">Inactive</Badge>
          )}
        </span>
      </button>
      <div className="flex flex-1 flex-col gap-1 p-3">
        <p className="text-[11px] font-medium tracking-wide text-forest-600">
          {product.articleNumber}
        </p>
        <button
          type="button"
          onClick={onOpen}
          className="text-left text-sm font-semibold text-neutral-900 hover:text-forest-700"
        >
          {product.name}
        </button>
        <p className="truncate text-xs text-neutral-500">{product.category}</p>
        <div className="flex flex-wrap items-center gap-2 pt-1 text-xs">
          <span className="font-medium text-neutral-800">{formatPkr(product.salePriceMinor)}</span>
          {canViewCost && product.costMinor !== null && (
            <span className="text-neutral-400">cost {formatPkr(product.costMinor)}</span>
          )}
        </div>
        {canMutate && (
          <div className="mt-auto flex gap-1 border-t border-neutral-100 pt-2 opacity-0 transition-opacity group-hover:opacity-100">
            <Button variant="ghost" size="sm" onClick={onEdit}>
              <Pencil className="h-3.5 w-3.5" />
              Edit
            </Button>
            <Button variant="ghost" size="sm" onClick={onDuplicate}>
              <Copy className="h-3.5 w-3.5" />
              Copy
            </Button>
            {product.archivedAt ? (
              <Button variant="ghost" size="sm" onClick={onUnarchive}>
                <ArchiveRestore className="h-3.5 w-3.5" />
                Restore
              </Button>
            ) : (
              <Button variant="ghost" size="sm" onClick={onArchive} aria-label="Archive product">
                <Archive className="h-3.5 w-3.5" />
              </Button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function ProductsTable({
  products,
  canViewCost,
  canMutate,
  onOpen,
  onEdit,
  onDuplicate,
  onArchive,
  onUnarchive,
}: {
  products: ProductListItemDto[];
  canViewCost: boolean;
  canMutate: boolean;
  onOpen: (p: ProductListItemDto) => void;
  onEdit: (p: ProductListItemDto) => void;
  onDuplicate: (p: ProductListItemDto) => void;
  onArchive: (p: ProductListItemDto) => void;
  onUnarchive: (p: ProductListItemDto) => void;
}) {
  return (
    <div className="mt-5 overflow-x-auto rounded-lg border border-neutral-200 bg-white">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Product</TableHead>
            <TableHead>Article</TableHead>
            <TableHead>Category</TableHead>
            <TableHead>Type</TableHead>
            <TableHead className="text-right">Sale price</TableHead>
            {canViewCost && <TableHead className="text-right">Cost</TableHead>}
            <TableHead>Stock</TableHead>
            <TableHead>Status</TableHead>
            {canMutate && <TableHead className="text-right">Actions</TableHead>}
          </TableRow>
        </TableHeader>
        <TableBody>
          {products.map((p) => (
            <TableRow key={p.id}>
              <TableCell>
                <button
                  type="button"
                  onClick={() => onOpen(p)}
                  className="flex items-center gap-2 text-left hover:text-forest-700"
                >
                  <span className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded bg-neutral-100">
                    <StoredImage path={p.primaryThumbnailPath} className="h-full w-full object-cover" />
                  </span>
                  <span className="text-sm font-medium text-neutral-900">{p.name}</span>
                </button>
              </TableCell>
              <TableCell className="text-xs font-medium text-forest-600">{p.articleNumber}</TableCell>
              <TableCell className="text-sm text-neutral-600">{p.category}</TableCell>
              <TableCell className="text-sm text-neutral-600">{p.productType ?? "—"}</TableCell>
              <TableCell className="text-right text-sm font-medium text-neutral-800">
                {formatPkr(p.salePriceMinor)}
              </TableCell>
              {canViewCost && (
                <TableCell className="text-right text-sm text-neutral-500">
                  {p.costMinor !== null ? formatPkr(p.costMinor) : "—"}
                </TableCell>
              )}
              <TableCell className="text-sm text-neutral-400">—</TableCell>
              <TableCell>
                {p.archivedAt ? (
                  <Badge variant="neutral">Archived</Badge>
                ) : p.isActive ? (
                  <Badge variant="accent">Active</Badge>
                ) : (
                  <Badge variant="warning">Inactive</Badge>
                )}
              </TableCell>
              {canMutate && (
                <TableCell>
                  <div className="flex justify-end gap-1">
                    <Button variant="ghost" size="sm" onClick={() => onEdit(p)}>
                      <Pencil className="h-3.5 w-3.5" />
                      Edit
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => onDuplicate(p)}>
                      <Copy className="h-3.5 w-3.5" />
                      Copy
                    </Button>
                    {p.archivedAt ? (
                      <Button variant="ghost" size="sm" onClick={() => onUnarchive(p)}>
                        <ArchiveRestore className="h-3.5 w-3.5" />
                        Restore
                      </Button>
                    ) : (
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onArchive(p)}
                        aria-label="Archive product"
                      >
                        <Archive className="h-3.5 w-3.5" />
                      </Button>
                    )}
                  </div>
                </TableCell>
              )}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
