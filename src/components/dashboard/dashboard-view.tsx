"use client";

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";

import { PageHeader } from "@/components/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useToast } from "@/components/ui/toast";
import {
  proofAppInfo,
  proofCreateBackup,
  proofGeneratePdf,
  proofImportImage,
  proofListBackups,
  proofOpenPath,
  type BackupResult,
  type ImportResult,
  type PdfResult,
} from "@/lib/tauri/api";
import type { CommandError } from "@/lib/tauri/client";
import { formatDateTime } from "@/lib/format";

function errorText(e: unknown): string {
  const err = e as CommandError;
  return err.code
    ? `${err.code}${err.correlationId ? ` · ${err.correlationId}` : ""}: ${err.message}`
    : String(e);
}

function Panel({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <section className="rounded-lg border bg-white p-5 shadow-sm">
      <h2 className="text-base font-semibold text-neutral-900">{title}</h2>
      <p className="mt-0.5 text-sm text-neutral-500">{description}</p>
      <div className="mt-4">{children}</div>
    </section>
  );
}

export function DashboardView() {
  const { toast } = useToast();
  const queryClient = useQueryClient();

  const infoQuery = useQuery({
    queryKey: ["appInfo"],
    queryFn: proofAppInfo,
  });

  const backupsQuery = useQuery({
    queryKey: ["backups"],
    queryFn: proofListBackups,
  });

  const [pdfBusy, setPdfBusy] = useState(false);
  const [pdf, setPdf] = useState<PdfResult | null>(null);
  const [image, setImage] = useState<ImportResult | null>(null);
  const [imageBusy, setImageBusy] = useState(false);
  const [backupBusy, setBackupBusy] = useState(false);

  const handlePdf = async () => {
    setPdfBusy(true);
    try {
      const result = await proofGeneratePdf();
      setPdf(result);
      toast({
        variant: "success",
        title: "PDF generated",
        description: `${result.pages} page(s), ${result.bytes.toLocaleString()} bytes`,
      });
    } catch (e) {
      toast({ variant: "error", title: "PDF failed", description: errorText(e) });
    } finally {
      setPdfBusy(false);
    }
  };

  const handlePickImage = async () => {
    setImageBusy(true);
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp"] }],
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      const result = await proofImportImage(selected);
      setImage(result);
      toast({
        variant: "success",
        title: "Image imported",
        description: `${result.width}×${result.height}, thumbnail stored`,
      });
    } catch (e) {
      toast({ variant: "error", title: "Import failed", description: errorText(e) });
    } finally {
      setImageBusy(false);
    }
  };

  const handleBackup = async () => {
    setBackupBusy(true);
    try {
      const result: BackupResult = await proofCreateBackup();
      void queryClient.invalidateQueries({ queryKey: ["backups"] });
      toast({
        variant: "success",
        title: result.verified ? "Backup verified" : "Backup saved",
        description: `${result.sha256.slice(0, 16)}…`,
      });
    } catch (e) {
      toast({ variant: "error", title: "Backup failed", description: errorText(e) });
    } finally {
      setBackupBusy(false);
    }
  };

  const handleOpenPdf = async () => {
    if (!pdf) return;
    try {
      await proofOpenPath(pdf.reportPath);
    } catch (e) {
      toast({ variant: "error", title: "Open failed", description: errorText(e) });
    }
  };

  return (
    <div>
      <PageHeader
        title="Dashboard"
        subtitle="System foundation: typed commands, migrations, image pipeline, PDF, backup."
        actions={
          <Button onClick={handleBackup} disabled={backupBusy}>
            {backupBusy ? "Backing up…" : "Back up now"}
          </Button>
        }
      />

      <div className="mt-5 space-y-5">
        <Panel
          title="Typed command + SQLite"
          description={
            infoQuery.data
              ? `Schema v${infoQuery.data.dbVersion} · ${infoQuery.data.pendingMigrations} pending migration(s)`
              : "Invokes a typed Tauri command into Rust."
          }
        >
          {infoQuery.isLoading && <p className="text-sm text-neutral-500">Loading app info…</p>}
          {infoQuery.isError && (
            <p className="text-sm text-red-700">{errorText(infoQuery.error)}</p>
          )}
          {infoQuery.data && (
            <div className="flex flex-wrap gap-2">
              <Badge variant="success">
                {infoQuery.data.dbVersion === 1 ? "Migrated" : "Pending"}
              </Badge>
              <Badge variant="neutral">{infoQuery.data.dbPath}</Badge>
            </div>
          )}
        </Panel>

        <Panel
          title="Unicode PDF (English + Urdu)"
          description="Generates an A4 PDF with PKR amounts and sample Urdu text using an embedded Unicode font."
        >
          <div className="flex flex-wrap gap-2">
            <Button onClick={handlePdf} disabled={pdfBusy}>
              {pdfBusy ? "Generating…" : "Generate PDF"}
            </Button>
            {pdf && !pdfBusy && (
              <Button variant="outline" onClick={handleOpenPdf}>
                Open
              </Button>
            )}
          </div>
          {pdf && (
            <p className="mt-3 text-xs text-neutral-500">
              {pdf.reportPath} · {pdf.pages} page(s) · {pdf.bytes.toLocaleString()} bytes
            </p>
          )}
        </Panel>

        <Panel
          title="Image import pipeline"
          description="Validates a photo, re-encodes it, generates a thumbnail, and stores it in app data."
        >
          <div className="flex flex-wrap items-center gap-3">
            <Button onClick={handlePickImage} disabled={imageBusy} variant="outline">
              {imageBusy ? "Importing…" : "Pick image"}
            </Button>
            {image && (
              <span className="text-xs text-neutral-500">
                {image.storedName} · {image.width}×{image.height} · thumbnail{" "}
                {image.thumbnailBytes} bytes
              </span>
            )}
          </div>
        </Panel>

        <Panel
          title="WAL-safe backup"
          description="Consistent SQLite snapshot via the backup API with SHA-256 and integrity verification."
        >
          {backupsQuery.isLoading && (
            <p className="text-sm text-neutral-500">Loading backups…</p>
          )}
          {backupsQuery.data && backupsQuery.data.length > 0 ? (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead className="text-right">Bytes</TableHead>
                  <TableHead>SHA-256</TableHead>
                  <TableHead>Created</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {backupsQuery.data.map((b) => (
                  <TableRow key={b.name}>
                    <TableCell className="font-medium">{b.name}</TableCell>
                    <TableCell className="text-right">{b.sizeBytes.toLocaleString()}</TableCell>
                    <TableCell className="font-mono text-xs">{b.sha256.slice(0, 16)}…</TableCell>
                    <TableCell>{formatDateTime(b.createdAt)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          ) : (
            <p className="text-sm text-neutral-500">No backups yet — use “Back up now” above.</p>
          )}
        </Panel>
      </div>
    </div>
  );
}