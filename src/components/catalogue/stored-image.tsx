"use client";

import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import { Loader2, Package } from "lucide-react";

import { cn } from "@/lib/utils";
import { useSession } from "@/components/session/session-provider";
import { productImageData } from "@/lib/tauri/api";

/**
 * Renders a stored product image. The backend has no `asset:` protocol enabled
 * (offline dependency policy), so the image bytes are fetched through the
 * `product_image_data` command as a `data:` URL (already allowed by the CSP).
 */
export function StoredImage({
  path,
  className,
  alt,
}: {
  path: string | null | undefined;
  className?: string;
  alt?: string;
}) {
  const { profile } = useSession();
  const session = profile?.sessionId ?? "";

  const { data, isError } = useQuery({
    queryKey: ["catalogue", "image", path],
    queryFn: () => productImageData(session, path as string),
    enabled: !!session && !!path,
    staleTime: 60_000,
  });

  if (!path) {
    return (
      <div className={cn("flex items-center justify-center bg-neutral-100", className)}>
        <Package className="h-10 w-10 text-neutral-300" />
      </div>
    );
  }
  if (!data) {
    return (
      <div className={cn("flex items-center justify-center bg-neutral-100", className)}>
        {isError ? null : <Loader2 className="h-4 w-4 animate-spin text-neutral-400" />}
      </div>
    );
  }
  // eslint-disable-next-line @next/next/no-img-element
  return <img src={data} alt={alt ?? ""} className={className} />;
}