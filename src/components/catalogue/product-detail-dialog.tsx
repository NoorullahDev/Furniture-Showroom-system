"use client";

import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Archive,
  ArchiveRestore,
  ChevronDown,
  ChevronUp,
  Copy,
  ImagePlus,
  Loader2,
  Maximize2,
  Pencil,
  RefreshCw,
  Star,
  Trash2,
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
import { useToast } from "@/components/ui/toast";
import { isSessionError, useSession } from "@/components/session/session-provider";
import {
  auditQuery,
  productArchive,
  productGet,
  productImageAdd,
  productImageRemove,
  productImageReorder,
  productImageSetPrimary,
  productUnarchive,
  type AuditEvent,
} from "@/lib/tauri/api";
import { formatDateTime, formatPkr } from "@/lib/format";
import { commandErrorMessage } from "@/lib/tauri/client";
import { StoredImage } from "@/components/catalogue/stored-image";
import { ProductEditorDialog } from "@/components/catalogue/product-editor-dialog";
import { DuplicateProductDialog } from "@/components/catalogue/duplicate-product-dialog";

export function ProductDetailDialog({
  productId,
  onClose,
}: {
  productId: number;
  onClose: () => void;
}) {
  const { toast } = useToast();
  const { refresh, profile, hasPermission } = useSession();
  const queryClient = useQueryClient();
  const session = profile?.sessionId ?? "";

  const canMutate = hasPermission("product.create");
  const canViewCost = hasPermission("product.cost.view");
  const canViewAudit = hasPermission("audit.view");

  const [tab, setTab] = React.useState<"details" | "activity">("details");
  const [editing, setEditing] = React.useState(false);
  const [duplicating, setDuplicating] = React.useState(false);
  const [picking, setPicking] = React.useState(false);
  const [replacingId, setReplacingId] = React.useState<number | null>(null);
  const [previewPath, setPreviewPath] = React.useState<string | null>(null);

  const detailQuery = useQuery({
    queryKey: ["catalogue", "product", productId],
    queryFn: () => productGet(session, productId),
    enabled: !!session,
  });

  const activityQuery = useQuery({
    queryKey: ["catalogue", "product", "activity", productId],
    queryFn: () =>
      auditQuery(session, { entityType: "product", entityId: String(productId), limit: 50 }),
    enabled: !!session && canViewAudit,
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["catalogue", "product", productId] });
    void queryClient.invalidateQueries({ queryKey: ["catalogue", "products"] });
    void queryClient.invalidateQueries({
      queryKey: ["catalogue", "product", "activity", productId],
    });
  };

  const archiveMutation = useMutation({
    mutationFn: () => productArchive(session, productId, "Archived from detail dialog"),
    onSuccess: () => {
      toast({ variant: "success", title: "Product archived" });
      invalidate();
      onClose();
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
    mutationFn: () => productUnarchive(session, productId),
    onSuccess: () => {
      toast({ variant: "success", title: "Product restored" });
      invalidate();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Restore failed", description: commandErrorMessage(e) });
    },
  });

  const addImageMutation = useMutation({
    mutationFn: (path: string) => productImageAdd(session, productId, path),
    onSuccess: () => {
      toast({ variant: "success", title: "Image added" });
      invalidate();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Add image failed", description: commandErrorMessage(e) });
    },
  });

  async function pickAndAddImage() {
    const selected = await open({
      multiple: false,
      title: "Add product image",
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"],
        },
      ],
    });
    if (typeof selected !== "string") return;
    addImageMutation.mutate(selected);
  }

  const removeImageMutation = useMutation({
    mutationFn: (imageId: number) => productImageRemove(session, productId, imageId),
    onSuccess: () => {
      toast({ variant: "success", title: "Image removed" });
      invalidate();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Remove image failed", description: commandErrorMessage(e) });
    },
  });

  const setPrimaryMutation = useMutation({
    mutationFn: (imageId: number) => productImageSetPrimary(session, productId, imageId),
    onSuccess: () => {
      toast({ variant: "success", title: "Primary image updated" });
      invalidate();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Update failed", description: commandErrorMessage(e) });
    },
  });

  const reorderMutation = useMutation({
    mutationFn: (orderedIds: number[]) => productImageReorder(session, productId, orderedIds),
    onSuccess: () => {
      toast({ variant: "success", title: "Image order updated" });
      invalidate();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Reorder failed", description: commandErrorMessage(e) });
    },
  });

  const replaceImageMutation = useMutation({
    mutationFn: async ({ imageId, newPath }: { imageId: number; newPath: string }) => {
      await productImageAdd(session, productId, newPath);
      await productImageRemove(session, productId, imageId);
    },
    onSuccess: () => {
      toast({ variant: "success", title: "Image replaced" });
      invalidate();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Replace failed", description: commandErrorMessage(e) });
    },
  });

  async function pickAndReplaceImage(imageId: number) {
    const selected = await open({
      multiple: false,
      title: "Replace product image",
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"],
        },
      ],
    });
    if (typeof selected !== "string") return;
    replaceImageMutation.mutate({ imageId, newPath: selected });
  }

  function moveImage(imageId: number, delta: -1 | 1) {
    if (!product) return;
    const sorted = [...product.images].sort((a, b) => a.sortOrder - b.sortOrder);
    const index = sorted.findIndex((img) => img.id === imageId);
    const target = index + delta;
    if (index < 0 || target < 0 || target >= sorted.length) return;
    const orderedIds = [...sorted.map((img) => img.id)];
    [orderedIds[index], orderedIds[target]] = [orderedIds[target], orderedIds[index]];
    reorderMutation.mutate(orderedIds);
  }

  const product = detailQuery.data;
  const busy =
    addImageMutation.isPending ||
    removeImageMutation.isPending ||
    setPrimaryMutation.isPending ||
    reorderMutation.isPending ||
    replaceImageMutation.isPending;

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent className="max-w-3xl">
        {detailQuery.isLoading && !product ? (
          <div className="flex items-center justify-center gap-2 py-10 text-neutral-500">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading…
          </div>
        ) : !product ? (
          <div className="py-10 text-center text-sm text-neutral-500">Product not found.</div>
        ) : (
          <>
            <DialogHeader>
              <DialogTitle>{product.name}</DialogTitle>
              <DialogDescription>
                {product.articleNumber} · {product.category}
                {product.archivedAt ? " · Archived" : product.isActive ? " · Active" : " · Inactive"}
              </DialogDescription>
            </DialogHeader>

            <div className="flex gap-1 border-b border-neutral-200">
              <TabButton active={tab === "details"} onClick={() => setTab("details")}>
                Details
              </TabButton>
              <TabButton active={tab === "activity"} onClick={() => setTab("activity")}>
                Activity
              </TabButton>
            </div>

            {tab === "details" ? (
              <div className="grid max-h-[52vh] grid-cols-1 gap-5 overflow-y-auto pr-1 md:grid-cols-[220px_1fr]">
                <ImageStrip
                  images={product.images}
                  busy={busy}
                  canMutate={canMutate}
                  addImageDisabled={picking || product.images.length >= 8}
                  onAdd={() => {
                    setPicking(true);
                    void pickAndAddImage().finally(() => setPicking(false));
                  }}
                  onRemove={(id) => {
                    if (window.confirm("Remove this image?")) removeImageMutation.mutate(id);
                  }}
                  onSetPrimary={setPrimaryMutation.mutate}
                  onMoveUp={(id) => moveImage(id, -1)}
                  onMoveDown={(id) => moveImage(id, 1)}
                  onPreview={(path) => setPreviewPath(path)}
                  onReplace={(id) => {
                    setReplacingId(id);
                    void pickAndReplaceImage(id).finally(() => setReplacingId(null));
                  }}
                  replacingId={replacingId}
                />

                <div className="grid gap-4">
                  <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
                    <Field label="Category">{product.category}</Field>
                    <Field label="Product type">{product.productType ?? "—"}</Field>
                    <Field label="Unit">{product.unit ?? "—"}</Field>
                    <Field label="Brand">{product.brand ?? "—"}</Field>
                    <Field label="Barcode">{product.barcode ?? "—"}</Field>
                    <Field label="Warranty">
                      {product.warrantyMonths != null ? `${product.warrantyMonths} mo` : "—"}
                    </Field>
                    <Field label="Material">{product.material ?? "—"}</Field>
                    <Field label="Color">{product.color ?? "—"}</Field>
                    <Field label="Dimensions">{product.dimensionsText ?? "—"}</Field>
                    <Field label="Track stock">{product.trackStock ? "Yes" : "No"}</Field>
                  </dl>

                  <dl className="grid grid-cols-2 gap-x-4 gap-y-2 rounded-md border border-neutral-200 bg-neutral-50 p-3 text-sm">
                    <Field label="Sale price">{formatPkr(product.salePriceMinor)}</Field>
                    {canViewCost && (
                      <Field label="Cost">
                        {product.costMinor !== null ? formatPkr(product.costMinor) : "—"}
                      </Field>
                    )}
                    <Field label="On hand">— (Phase 4)</Field>
                  </dl>

                  {product.description && (
                    <div className="text-sm text-neutral-700">
                      <h4 className="font-medium text-neutral-900">Description</h4>
                      <p className="mt-1 whitespace-pre-wrap">{product.description}</p>
                    </div>
                  )}
                  {product.notes && (
                    <div className="text-sm text-neutral-700">
                      <h4 className="font-medium text-neutral-900">Notes</h4>
                      <p className="mt-1 whitespace-pre-wrap">{product.notes}</p>
                    </div>
                  )}

                  {product.attributes.length > 0 && (
                    <div>
                      <h4 className="text-sm font-medium text-neutral-900">Attributes</h4>
                      <dl className="mt-1 grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
                        {product.attributes.map((a, i) => (
                          <React.Fragment key={`${a.name}-${i}`}>
                            <dt className="text-neutral-500">{a.name}</dt>
                            <dd className="text-neutral-800">{a.value}</dd>
                          </React.Fragment>
                        ))}
                      </dl>
                    </div>
                  )}

                  <p className="text-xs text-neutral-400">
                    Created {formatDateTime(product.createdAt)} · Updated{" "}
                    {formatDateTime(product.updatedAt)}
                  </p>
                </div>
              </div>
            ) : (
              <ActivityList
                events={activityQuery.data?.items ?? []}
                loading={activityQuery.isLoading}
                canViewAudit={canViewAudit}
              />
            )}

            <DialogFooter>
              {canMutate && (
                <>
                  <Button variant="outline" onClick={() => setDuplicating(true)}>
                    <Copy className="h-4 w-4" />
                    Duplicate
                  </Button>
                  {product.archivedAt ? (
                    <Button variant="outline" onClick={() => unarchiveMutation.mutate()}>
                      <ArchiveRestore className="h-4 w-4" />
                      Restore
                    </Button>
                  ) : (
                    <Button
                      variant="outline"
                      onClick={() => {
                        if (window.confirm(`Archive “${product.name}”?`)) archiveMutation.mutate();
                      }}
                    >
                      <Archive className="h-4 w-4" />
                      Archive
                    </Button>
                  )}
                  <Button onClick={() => setEditing(true)}>
                    <Pencil className="h-4 w-4" />
                    Edit
                  </Button>
                </>
              )}
            </DialogFooter>
          </>
        )}
      </DialogContent>

{editing && product && (
          <ProductEditorDialog
            productId={product.id}
            onClose={() => setEditing(false)}
            onSaved={invalidate}
          />
        )}
        {duplicating && product && (
          <DuplicateProductDialog
            product={product}
            onClose={() => setDuplicating(false)}
            onSaved={invalidate}
          />
        )}
        <Dialog open={!!previewPath} onOpenChange={() => setPreviewPath(null)}>
          <DialogContent className="max-w-4xl">
            <DialogHeader>
              <DialogTitle>Image preview</DialogTitle>
            </DialogHeader>
            {previewPath && (
              <div className="flex max-h-[75vh] items-center justify-center overflow-auto">
                <StoredImage path={previewPath} className="max-h-[75vh] w-auto object-contain" />
              </div>
            )}
          </DialogContent>
        </Dialog>
      </Dialog>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <dt className="text-neutral-500">{label}</dt>
      <dd className="text-neutral-900">{children}</dd>
    </>
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
      className={`-mb-px border-b-2 px-3 py-2 text-sm font-medium transition-colors ${
        active
          ? "border-forest-600 text-forest-700"
          : "border-transparent text-neutral-500 hover:text-forest-700"
      }`}
    >
      {children}
    </button>
  );
}

function ActivityList({
  events,
  loading,
  canViewAudit,
}: {
  events: AuditEvent[];
  loading: boolean;
  canViewAudit: boolean;
}) {
  if (!canViewAudit) {
    return (
      <p className="rounded-md border border-dashed border-neutral-300 p-4 text-center text-xs text-neutral-400">
        Activity is hidden — you need the &quot;View audit log&quot; permission.
      </p>
    );
  }
  if (loading && events.length === 0) {
    return (
      <div className="flex items-center justify-center gap-2 py-10 text-neutral-500">
        <Loader2 className="h-4 w-4 animate-spin" />
        Loading activity…
      </div>
    );
  }
  if (events.length === 0) {
    return (
      <p className="rounded-md border border-dashed border-neutral-300 p-4 text-center text-xs text-neutral-400">
        No activity recorded for this product yet.
      </p>
    );
  }
  return (
    <ul className="max-h-[52vh] divide-y divide-neutral-100 overflow-y-auto pr-1">
      {events.map((e) => (
        <li key={e.id} className="grid gap-0.5 py-2">
          <div className="flex items-baseline justify-between gap-2">
            <span className="font-mono text-xs font-medium text-neutral-800">{e.action}</span>
            <span className="shrink-0 text-[10px] text-neutral-400">
              {formatDateTime(e.createdAt)}
            </span>
          </div>
          <div className="flex items-center gap-2 text-[10px] text-neutral-400">
            <span>#id {e.id}</span>
            {e.userId != null && <span>user #{e.userId}</span>}
            {e.entityId != null && <span>product #{e.entityId}</span>}
          </div>
          {e.reason && <p className="text-xs text-neutral-500">{e.reason}</p>}
        </li>
      ))}
    </ul>
  );
}

function ImageStrip({
  images,
  busy,
  canMutate,
  addImageDisabled,
  onAdd,
  onRemove,
  onSetPrimary,
  onMoveUp,
  onMoveDown,
  onPreview,
  onReplace,
  replacingId,
}: {
  images: { id: number; imagePath: string; thumbnailPath: string; sortOrder: number; isPrimary: boolean }[];
  busy: boolean;
  canMutate: boolean;
  addImageDisabled: boolean;
  onAdd: () => void;
  onRemove: (id: number) => void;
  onSetPrimary: (id: number) => void;
  onMoveUp: (id: number) => void;
  onMoveDown: (id: number) => void;
  onPreview: (path: string) => void;
  onReplace: (id: number) => void;
  replacingId: number | null;
}) {
  const sortedImages = [...images].sort((a, b) => a.sortOrder - b.sortOrder);
  return (
    <div className="grid gap-2">
      {sortedImages.map((img, i) => (
          <div key={img.id} className="group relative flex items-center gap-2 rounded-md border border-neutral-200 bg-white p-1.5">
            <button
              type="button"
              onClick={() => onPreview(img.imagePath)}
              className="flex h-14 w-14 shrink-0 cursor-zoom-in items-center justify-center overflow-hidden rounded bg-neutral-100 hover:ring-2 hover:ring-forest-500/50"
              aria-label={`Preview image #${img.sortOrder + 1}`}
            >
              <StoredImage path={img.thumbnailPath} className="h-full w-full object-cover" />
            </button>
            <div className="min-w-0 flex-1">
              <p className="truncate text-xs font-medium text-neutral-800">
                #{img.sortOrder + 1} {img.isPrimary && <Star className="inline h-3 w-3 text-amber-500" />}
              </p>
              <p className="truncate text-[10px] text-neutral-400">Image #{img.id}</p>
            </div>
            {canMutate && (
              <div className="grid shrink-0 gap-1">
                <div className="flex gap-1">
                  {!img.isPrimary && (
                    <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => onSetPrimary(img.id)} disabled={busy} title="Set as primary">
                      <Star className="h-3.5 w-3.5" />
                    </Button>
                  )}
                  <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => onRemove(img.id)} disabled={busy} title="Remove">
                    <Trash2 className="h-3.5 w-3.5 text-red-600" />
                  </Button>
                </div>
                <div className="flex justify-center gap-1">
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-5 w-5"
                    onClick={() => onMoveUp(img.id)}
                    disabled={busy || i === 0}
                    aria-label="Move image up"
                  >
                    <ChevronUp className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-5 w-5"
                    onClick={() => onMoveDown(img.id)}
                    disabled={busy || i === sortedImages.length - 1}
                    aria-label="Move image down"
                  >
                    <ChevronDown className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-5 w-5"
                    onClick={() => onReplace(img.id)}
                    disabled={busy || replacingId === img.id}
                    aria-label="Replace image"
                    title="Replace"
                  >
                    <RefreshCw className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-5 w-5"
                    onClick={() => onPreview(img.imagePath)}
                    aria-label="Preview full image"
                  >
                    <Maximize2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
              </div>
            )}
          </div>
      ))}
      {canMutate && (
        <Button variant="outline" size="sm" onClick={onAdd} disabled={busy || addImageDisabled}>
          <ImagePlus className="h-4 w-4" />
          Add image
        </Button>
      )}
      {images.length === 0 && !canMutate && (
        <p className="rounded-md border border-dashed border-neutral-300 p-4 text-center text-xs text-neutral-400">
          No images
        </p>
      )}
    </div>
  );
}