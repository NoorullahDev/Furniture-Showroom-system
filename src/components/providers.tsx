"use client";

import * as React from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { ToastProvider } from "@/components/ui/toast";
import { SessionProvider } from "@/components/session/session-provider";
import { BackupCloseGuard } from "@/components/maintenance/backup-close-guard";
import { LicenseGate } from "@/components/license/license-gate";

function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 30_000,
        refetchOnWindowFocus: false,
        retry: 1,
      },
    },
  });
}

export function Providers({ children }: { children: React.ReactNode }) {
  const [client] = React.useState(makeQueryClient);

  return (
    <QueryClientProvider client={client}>
      <ToastProvider>
        <LicenseGate>
          <SessionProvider>
            {children}
            <BackupCloseGuard />
          </SessionProvider>
        </LicenseGate>
      </ToastProvider>
    </QueryClientProvider>
  );
}
