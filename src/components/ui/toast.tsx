"use client";

import * as React from "react";
import * as ToastPrimitive from "@radix-ui/react-toast";
import { AlertTriangle, CheckCircle2, Info, X } from "lucide-react";

import { cn } from "@/lib/utils";

const ToastProviderPrimitive = ToastPrimitive.Provider;
const ToastViewport = React.forwardRef<
  React.ElementRef<typeof ToastPrimitive.Viewport>,
  React.ComponentPropsWithoutRef<typeof ToastPrimitive.Viewport>
>(({ className, ...props }, ref) => (
  <ToastPrimitive.Viewport
    ref={ref}
    className={cn(
      "fixed bottom-0 right-0 z-[100] flex max-h-screen w-full flex-col-reverse gap-2 p-4 sm:max-w-sm",
      className,
    )}
    {...props}
  />
));
ToastViewport.displayName = ToastPrimitive.Viewport.displayName;

type ToastVariant = "default" | "success" | "error";
const variantIcons = {
  default: Info,
  success: CheckCircle2,
  error: AlertTriangle,
};

type ToastContextValue = {
  toast: (opts: { title: string; description?: string; variant?: ToastVariant }) => void;
};

const ToastContext = React.createContext<ToastContextValue | null>(null);

export function useToast(): ToastContextValue {
  const ctx = React.useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used inside <ToastProvider>");
  return ctx;
}

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = React.useState(false);
  const [item, setItem] = React.useState<{
    title: string;
    description?: string;
    variant: ToastVariant;
  } | null>(null);

  const toast = React.useCallback(
    (opts: { title: string; description?: string; variant?: ToastVariant }) => {
      setItem({ variant: "default", ...opts });
      setOpen(true);
    },
    [],
  );

  const Icon = item ? variantIcons[item.variant] : Info;

  return (
    <ToastContext.Provider value={{ toast }}>
      <ToastProviderPrimitive swipeDirection="right">
        {children}
        {item && (
          <ToastPrimitive.Root
            open={open}
            onOpenChange={setOpen}
            className={cn(
              "pointer-events-auto relative rounded-md border bg-white p-4 shadow-lg",
              "data-[state=open]:animate-in data-[state=closed]:animate-out data-[swipe=end]:animate-out",
              "data-[state=closed]:fade-out-80 data-[state=open]:slide-in-from-right-full",
              "data-[swipe=end]:translate-x-[var(--radix-toast-swipe-end-x)]",
              item.variant === "success" && "border-emerald-200",
              item.variant === "error" && "border-red-200",
            )}
          >
            <ToastPrimitive.Close className="absolute right-2 top-2 rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-600 focus:outline-none focus-visible:ring-2 focus-visible:ring-forest-500/60">
              <X className="h-3.5 w-3.5" />
              <span className="sr-only">Dismiss</span>
            </ToastPrimitive.Close>
            <div className="flex items-start gap-3 pr-6">
              <Icon
                className={cn(
                  "mt-0.5 h-4 w-4 shrink-0",
                  item.variant === "success" && "text-emerald-600",
                  item.variant === "error" && "text-red-600",
                  item.variant === "default" && "text-forest-600",
                )}
              />
              <div className="grid gap-1">
                <ToastPrimitive.Title className="text-sm font-semibold text-neutral-900">
                  {item.title}
                </ToastPrimitive.Title>
                {item.description && (
                  <ToastPrimitive.Description className="text-xs text-neutral-500">
                    {item.description}
                  </ToastPrimitive.Description>
                )}
              </div>
            </div>
          </ToastPrimitive.Root>
        )}
        <ToastViewport />
      </ToastProviderPrimitive>
    </ToastContext.Provider>
  );
}