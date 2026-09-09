"use client";

import * as React from "react";
import { useMutation } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";

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
import { useToast } from "@/components/ui/toast";
import { isSessionError, useSession } from "@/components/session/session-provider";
import { productDuplicate } from "@/lib/tauri/api";
import { commandErrorMessage } from "@/lib/tauri/client";

export function DuplicateProductDialog({
  product,
  onClose,
  onSaved,
}: {
  product: { id: number; articleNumber: string; name: string };
  onClose: () => void;
  onSaved: () => void;
}) {
  const { toast } = useToast();
  const { refresh, profile } = useSession();
  const session = profile?.sessionId ?? "";
  const [article, setArticle] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => productDuplicate(session, product.id, article),
    onSuccess: () => {
      toast({ variant: "success", title: `Copied as “${article}”` });
      onSaved();
      onClose();
    },
    onError: (e: unknown) => {
      if (isSessionError(e)) {
        refresh();
        return;
      }
      toast({ variant: "error", title: "Duplicate failed", description: commandErrorMessage(e) });
    },
  });

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!article.trim()) {
      setError("Enter the new article number.");
      return;
    }
    setError(null);
    mutation.mutate();
  }

  return (
    <Dialog open onOpenChange={onClose}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Duplicate “{product.name}”</DialogTitle>
          <DialogDescription>
            Creates a new product copying pricing, stock settings, attributes and images. You need a
            new article number — it must not already be in use.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="grid gap-4">
          <div className="grid gap-1.5">
            <Label htmlFor="dup-article">New article number *</Label>
            <Input
              id="dup-article"
              value={article}
              onChange={(e) => setArticle(e.target.value)}
              placeholder={product.articleNumber}
              autoFocus
            />
          </div>
          {error && (
            <p role="alert" className="rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-700">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={mutation.isPending}>
              Cancel
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending && <Loader2 className="h-4 w-4 animate-spin" />}
              Duplicate
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}