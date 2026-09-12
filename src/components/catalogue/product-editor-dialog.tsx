"use client";

import * as React from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Loader2, Plus, Trash2, Upload, X } from "lucide-react";

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
  categoryList,
  productCreate,
  productGet,
  productImageAdd,
  productImageRemove,
  productTypeList,
  productUpdate,
  unitList,
  type AttributeInputDto,
} from "@/lib/tauri/api";
import { formatPkr } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { StoredImage } from "@/components/catalogue/stored-image";

export function ProductEditorDialog({
  productId,
  onClose,
  onSaved,
}: {
  productId?: number;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const session = profile?.sessionId ?? "";
  const isEdit = productId !== undefined;
  const canViewCost = hasPermission("product.cost.view");

  const existing = useQuery({
    queryKey: ["catalogue", "product", productId],
    queryFn: () => productGet(session, productId as number),
    enabled: isEdit,
  });

  const categoriesQuery = useQuery({
    queryKey: ["catalogue", "categories"],
    queryFn: () => categoryList(session),
    enabled: !!session,
  });
  const unitsQuery = useQuery({
    queryKey: ["catalogue", "units"],
    queryFn: () => unitList(session),
    enabled: !!session,
  });

  const [categoryId, setCategoryId] = React.useState<number | null>(null);
  const typesQuery = useQuery({
    queryKey: ["catalogue", "types", categoryId],
    queryFn: () => productTypeList(session, categoryId),
    enabled: !!session && categoryId !== null,
  });

  const categories = categoriesQuery.data ?? [];
  const units = unitsQuery.data ?? [];
  const types = typesQuery.data ?? [];

  const [articleNumber, setArticleNumber] = React.useState("");
  const [name, setName] = React.useState("");
  const [productTypeId, setProductTypeId] = React.useState<number | null>(null);
  const [unitId, setUnitId] = React.useState<number | null>(null);
  const [salePriceMinor, setSalePriceMinor] = React.useState(0);
  const [costMinor, setCostMinor] = React.useState(0);
  const [minimumStock, setMinimumStock] = React.useState(0);
  const [trackStock, setTrackStock] = React.useState(true);
  const [material, setMaterial] = React.useState("");
  const [color, setColor] = React.useState("");
  const [dimensionsText, setDimensionsText] = React.useState("");
  const [brand, setBrand] = React.useState("");
  const [barcode, setBarcode] = React.useState("");
  const [warrantyMonths, setWarrantyMonths] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [notes, setNotes] = React.useState("");
  const [attributes, setAttributes] = React.useState<AttributeInputDto[]>([]);
  const [imagePaths, setImagePaths] = React.useState<string[]>([]);
  const [removedImages, setRemovedImages] = React.useState<number[]>([]);
  const [error, setError] = React.useState<string | null>(null);
  const categoryChanged = isEdit && existing.data ? categoryId !== existing.data.categoryId : true;
  const selectableCategories = categories.filter((c) => c.isActive || c.id === categoryId);
  const selectableTypes = types.filter((t) => t.isActive || t.id === productTypeId);
  const showTypeClear = !isEdit || (!categoryChanged && existing.data && productTypeId !== existing.data.productTypeId);
  const showUnitClear = !isEdit || (existing.data && existing.data.unitId === null);

  const isLoaded = !isEdit || !!existing.data;

  React.useEffect(() => {
    const product = existing.data;
    if (!product) return;
    setArticleNumber(product.articleNumber);
    setName(product.name);
    setCategoryId(product.categoryId);
    setProductTypeId(product.productTypeId);
    setUnitId(product.unitId);
    setSalePriceMinor(product.salePriceMinor);
    setCostMinor(product.costMinor ?? 0);
    setMinimumStock(product.minimumStock);
    setTrackStock(product.trackStock);
    setMaterial(product.material ?? "");
    setColor(product.color ?? "");
    setDimensionsText(product.dimensionsText ?? "");
    setBrand(product.brand ?? "");
    setBarcode(product.barcode ?? "");
    setWarrantyMonths(product.warrantyMonths != null ? String(product.warrantyMonths) : "");
    setDescription(product.description ?? "");
    setNotes(product.notes ?? "");
    setAttributes(product.attributes);
  }, [existing.data]);

  const saveMutation = useMutation({
    mutationFn: async () => {
        if (isEdit && existing.data) {
          const detail = await productUpdate(session, existing.data.id, {
            articleNumber,
            name,
            categoryId: categoryChanged ? (categoryId as number) : undefined,
            productTypeId:
              categoryChanged || productTypeId !== existing.data.productTypeId
                ? (productTypeId ?? undefined)
                : undefined,
            unitId: unitId !== existing.data.unitId ? (unitId ?? undefined) : undefined,
            description: description || null,
            material: material || null,
            color: color || null,
            dimensionsText: dimensionsText || null,
            brand: brand || null,
            barcode: barcode || null,
            warrantyMonths: warrantyMonths !== "" ? Number(warrantyMonths) : null,
            notes: notes || null,
            costMinor: canViewCost ? costMinor : null,
            salePriceMinor,
            minimumStock,
            trackStock,
            attributes,
          });
          
          for (const id of removedImages) {
            await productImageRemove(session, detail.id, id);
          }
          for (const path of imagePaths) {
            await productImageAdd(session, detail.id, path);
          }
          return detail;
        }
      return productCreate(session, {
        articleNumber,
        name,
        categoryId: categoryId as number,
        productTypeId: productTypeId ?? null,
        unitId: unitId ?? null,
        description: description || null,
        material: material || null,
        color: color || null,
        dimensionsText: dimensionsText || null,
        brand: brand || null,
        barcode: barcode || null,
        warrantyMonths: warrantyMonths !== "" ? Number(warrantyMonths) : null,
        notes: notes || null,
        costMinor: canViewCost ? costMinor : 0,
        salePriceMinor,
        minimumStock,
        trackStock,
        attributes,
        imagePaths,
      });
    },
    onSuccess: () => {
      toast({
        variant: "success",
        title: isEdit ? "Product updated" : "Product created",
      });
      onSaved();
      onClose();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({
        variant: "error",
        title: isEdit ? "Update failed" : "Create failed",
        description: commandErrorMessage(e),
      });
    },
  });

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!articleNumber.trim()) {
      setError("Article number is required.");
      return;
    }
    if (!name.trim()) {
      setError("Product name is required.");
      return;
    }
    if (!isEdit && categoryId === null) {
      setError("Choose a category.");
      return;
    }
    setError(null);
    saveMutation.mutate();
  }

  const pickImages = async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp"] }],
    });
    if (!selected) return;
    const list = Array.isArray(selected) ? selected : [selected];
    setImagePaths((prev) => [...prev, ...list]);
  };

  const existingImages = (existing.data?.images || []).filter(img => !removedImages.includes(img.id));
  const totalImageCount = existingImages.length + imagePaths.length;

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{isEdit ? "Edit product" : "New product"}</DialogTitle>
          <DialogDescription>
            Only shop owners and staff with the &quot;Create products&quot; permission can make
            changes.
          </DialogDescription>
        </DialogHeader>

        {!isLoaded ? (
          <div className="flex items-center justify-center gap-2 py-10 text-neutral-500">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading…
          </div>
        ) : (
          <form onSubmit={submit} className="grid max-h-[60vh] gap-4 overflow-y-auto pr-1">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div className="grid gap-1.5">
                <Label htmlFor="pe-article">Article number *</Label>
                <Input
                  id="pe-article"
                  value={articleNumber}
                  onChange={(e) => setArticleNumber(e.target.value)}
                  placeholder="e.g. A-1001"
                  autoFocus={!isEdit}
                />
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="pe-name">Name *</Label>
                <Input
                  id="pe-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g. Oak Dining Chair"
                />
              </div>
            </div>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
              <div className="grid gap-1.5">
                <Label>Category</Label>
                <Select
                  value={categoryId !== null ? String(categoryId) : "none"}
                  onValueChange={(v) => {
                    const id = v === "none" ? null : Number(v);
                    setCategoryId(id);
                    setProductTypeId(null);
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select category" />
                  </SelectTrigger>
                  <SelectContent>
                    {!isEdit && <SelectItem value="none">—</SelectItem>}
                    {selectableCategories.map((c) => (
                      <SelectItem key={c.id} value={String(c.id)}>
                        {c.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="grid gap-1.5">
                <Label>Product type</Label>
                <Select
                  value={productTypeId !== null ? String(productTypeId) : "none"}
                  onValueChange={(v) => setProductTypeId(v === "none" ? null : Number(v))}
                  disabled={categoryId === null}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Optional" />
                  </SelectTrigger>
                  <SelectContent>
                    {showTypeClear && <SelectItem value="none">—</SelectItem>}
                    {selectableTypes.map((t) => (
                      <SelectItem key={t.id} value={String(t.id)}>
                        {t.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="grid gap-1.5">
                <Label>Unit</Label>
                <Select
                  value={unitId !== null ? String(unitId) : "none"}
                  onValueChange={(v) => setUnitId(v === "none" ? null : Number(v))}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Optional" />
                  </SelectTrigger>
                  <SelectContent>
                    {showUnitClear && <SelectItem value="none">—</SelectItem>}
                    {units.map((u) => (
                      <SelectItem key={u.id} value={String(u.id)}>
                        {u.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-4 rounded-md border border-neutral-200 bg-neutral-50 p-3 sm:grid-cols-2">
              <div className="grid gap-1.5">
                <Label htmlFor="pe-price">Sale price *</Label>
                <MoneyInput
                  id="pe-price"
                  value={salePriceMinor}
                  onCommit={setSalePriceMinor}
                  aria-describedby="pe-price-hint"
                />
                <p className="text-[10px] text-neutral-400" id="pe-price-hint">
                  {formatPkr(salePriceMinor)}
                </p>
              </div>
              {canViewCost && (
                <div className="grid gap-1.5">
                  <Label htmlFor="pe-cost">Cost price</Label>
                  <MoneyInput id="pe-cost" value={costMinor} onCommit={setCostMinor} />
                </div>
              )}
              <div className="grid gap-1.5">
                <Label htmlFor="pe-min-stock">Minimum stock</Label>
                <Input
                  id="pe-min-stock"
                  inputMode="numeric"
                  value={minimumStock}
                  onChange={(e) => setMinimumStock(Number(e.target.value) || 0)}
                />
              </div>
              <div className="grid gap-1.5">
                <Label>Track stock</Label>
                <Select value={trackStock ? "yes" : "no"} onValueChange={(v) => setTrackStock(v === "yes")}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="yes">Yes</SelectItem>
                    <SelectItem value="no">No</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div className="grid gap-1.5">
                <Label htmlFor="pe-material">Material</Label>
                <Input id="pe-material" value={material} onChange={(e) => setMaterial(e.target.value)} />
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="pe-color">Color / finish</Label>
                <Input id="pe-color" value={color} onChange={(e) => setColor(e.target.value)} />
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="pe-dimensions">Dimensions</Label>
                <Input id="pe-dimensions" value={dimensionsText} onChange={(e) => setDimensionsText(e.target.value)} placeholder="e.g. 50 × 45 × 80 cm" />
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="pe-brand">Brand</Label>
                <Input id="pe-brand" value={brand} onChange={(e) => setBrand(e.target.value)} />
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="pe-barcode">Barcode</Label>
                <Input id="pe-barcode" value={barcode} onChange={(e) => setBarcode(e.target.value)} />
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="pe-warranty">Warranty (months)</Label>
                <Input
                  id="pe-warranty"
                  inputMode="numeric"
                  value={warrantyMonths}
                  onChange={(e) => setWarrantyMonths(e.target.value.replace(/[^0-9]/g, ""))}
                />
              </div>
            </div>

            <div className="grid gap-1.5">
              <Label htmlFor="pe-desc">Description</Label>
              <textarea
                id="pe-desc"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                rows={2}
                className="rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm text-neutral-900 shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-forest-500/60"
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="pe-notes">Notes</Label>
              <textarea
                id="pe-notes"
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                rows={2}
                className="rounded-md border border-neutral-300 bg-white px-3 py-2 text-sm text-neutral-900 shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-forest-500/60"
              />
            </div>

            <div className="grid gap-1.5">
              <Label>Attributes</Label>
              {attributes.map((a, i) => (
                <div key={i} className="flex items-center gap-2">
                  <Input
                    value={a.name}
                    onChange={(e) =>
                      setAttributes((prev) =>
                        prev.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)),
                      )
                    }
                    placeholder="Name (e.g. Seat height)"
                    className="w-1/3"
                  />
                  <Input
                    value={a.value}
                    onChange={(e) =>
                      setAttributes((prev) =>
                        prev.map((x, j) => (j === i ? { ...x, value: e.target.value } : x)),
                      )
                    }
                    placeholder="Value (e.g. 45 cm)"
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onClick={() => setAttributes((prev) => prev.filter((_, j) => j !== i))}
                  >
                    <Trash2 className="h-4 w-4 text-red-600" />
                  </Button>
                </div>
              ))}
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => setAttributes((prev) => [...prev, { name: "", value: "" }])}
              >
                <Plus className="h-4 w-4" />
                Add attribute
              </Button>
            </div>

            <div className="grid gap-1.5">
              <Label>Images ({totalImageCount})</Label>
              {totalImageCount > 0 && (
                <ul className="grid grid-cols-2 gap-2 sm:grid-cols-4">
                  {existingImages.map((img) => (
                    <li key={img.id} className="relative aspect-square overflow-hidden rounded-md border border-neutral-200 bg-neutral-100">
                      <StoredImage path={img.imagePath} className="h-full w-full object-cover" />
                      <button
                        type="button"
                        onClick={() => setRemovedImages((prev) => [...prev, img.id])}
                        className="absolute right-1 top-1 rounded-full bg-white/80 p-1 text-neutral-600 hover:text-red-600"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </li>
                  ))}
                  {imagePaths.map((p) => (
                    <li key={p} className="relative aspect-square overflow-hidden rounded-md border border-neutral-200 bg-neutral-100">
                      {/* eslint-disable-next-line @next/next/no-img-element */}
                      <img src={convertFileSrc(p)} alt="Upload preview" className="h-full w-full object-cover" />
                      <button
                        type="button"
                        onClick={() => setImagePaths((prev) => prev.filter((x) => x !== p))}
                        className="absolute right-1 top-1 rounded-full bg-white/80 p-1 text-neutral-600 hover:text-red-600"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <Button type="button" variant="outline" size="sm" onClick={pickImages}>
                <Upload className="mr-2 h-4 w-4" />
                Choose image files
              </Button>
            </div>

            {error && (
              <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700">
                {error}
              </p>
            )}

            <DialogFooter>
              <Button type="button" variant="outline" onClick={onClose} disabled={saveMutation.isPending}>
                Cancel
              </Button>
              <Button type="submit" disabled={saveMutation.isPending}>
                {saveMutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
                {isEdit ? "Save changes" : "Create product"}
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}