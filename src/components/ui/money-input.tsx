"use client";

import * as React from "react";

import { formatPkrInput, parsePkrInput } from "@/lib/format";
import { cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";

interface MoneyInputProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "value" | "onChange"> {
  /** Value in integer minor units (paisa). */
  value: number;
  /** Commits parsed minor units (intended for blur or form submit). */
  onCommit: (minor: number) => void;
  className?: string;
}

const MoneyInput = React.forwardRef<HTMLInputElement, MoneyInputProps>(
  ({ value, onCommit, className, ...props }, ref) => {
    const [text, setText] = React.useState(() => formatPkrInput(value));

    React.useEffect(() => {
      setText(formatPkrInput(value));
    }, [value]);

    const handleBlur = () => {
      onCommit(parsePkrInput(text));
    };

    return (
      <Input
        ref={ref}
        inputMode="decimal"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={handleBlur}
        className={cn("tabular", className)}
        aria-describedby={props["aria-describedby"]}
        {...props}
      />
    );
  },
);
MoneyInput.displayName = "MoneyInput";

export { MoneyInput };