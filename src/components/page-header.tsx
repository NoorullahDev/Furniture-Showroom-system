import * as React from "react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

interface PageHeaderProps extends React.HTMLAttributes<HTMLDivElement> {
  title: string;
  subtitle?: string;
  actions?: React.ReactNode;
}

export function PageHeader({
  title,
  subtitle,
  actions,
  className,
  ...props
}: PageHeaderProps) {
  return (
    <div
      className={cn(
        "flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between",
        className,
      )}
      {...props}
    >
      <div className="grid min-w-0 gap-0.5">
        <h1 className="text-xl font-bold tracking-tight text-slate-900">{title}</h1>
        {subtitle && <p className="text-xs text-slate-500">{subtitle}</p>}
      </div>
      {actions && <div className="flex min-w-0 flex-wrap items-center gap-2 lg:justify-end">{actions}</div>}
    </div>
  );
}

/** Demo empty-actions helper so pages can add the primary action easily. */
export function PrimaryAction({
  children,
  onClick,
}: {
  children: React.ReactNode;
  onClick?: () => void;
}) {
  return <Button onClick={onClick}>{children}</Button>;
}
