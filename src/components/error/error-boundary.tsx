"use client";

import * as React from "react";

import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";

export function FatalErrorView({
  title = "The application could not start",
  message,
  correlationId,
}: {
  title?: string;
  message?: string;
  correlationId?: string;
}) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-cream p-6">
      <div className="w-full max-w-md rounded-lg border border-red-200 bg-white p-6 shadow-sm">
        <div className="flex items-start gap-3">
          <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-red-600" />
          <div className="grid gap-2">
            <h1 className="text-base font-semibold text-neutral-900">{title}</h1>
            <p className="text-sm text-neutral-600">
              {message ??
                "An unexpected error occurred. Restart the application; if the problem persists, contact support with the correlation id below."}
            </p>
            {correlationId && (
              <p className="rounded bg-neutral-50 border border-neutral-200 px-2 py-1 font-mono text-xs text-neutral-500 break-all">
                correlation: {correlationId}
              </p>
            )}
          </div>
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="outline" onClick={() => window.location.reload()}>
            Reload
          </Button>
        </div>
      </div>
    </div>
  );
}

type ErrorBoundaryProps = {
  children: React.ReactNode;
};

type ErrorBoundaryState = {
  hasError: boolean;
  message: string;
  correlationId: string;
};

export class ErrorBoundary extends React.Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = {
    hasError: false,
    message: "",
    correlationId: "",
  };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    return {
      hasError: true,
      message: error instanceof Error ? error.message : String(error),
      correlationId: crypto.randomUUID(),
    };
  }

  componentDidCatch(error: unknown, info: React.ErrorInfo) {
    // Phase 1: no async reporter; the FatalErrorView surfaces correlation id.
    console.error("[furniture-shop] render error", error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <FatalErrorView
          title="Something went wrong"
          message={this.state.message}
          correlationId={this.state.correlationId}
        />
      );
    }
    return this.props.children;
  }
}