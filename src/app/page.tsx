"use client";

import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { runCommand, type CommandError } from "@/lib/tauri/client";

type AppInfo = {
  appName: string;
  version: string;
  dataDir: string;
  dbPath: string;
  dbVersion: number;
  pendingMigrations: number;
};

type PdfResult = {
  reportPath: string;
  pages: number;
  bytes: number;
};

type ImportResult = {
  originalPath: string;
  storedName: string;
  width: number;
  height: number;
  thumbnailBytes: number;
};

type BackupResult = {
  backupPath: string;
  sha256: string;
  verified: boolean;
  bytes: number;
};

type BackupEntry = {
  name: string;
  sizeBytes: number;
  sha256: string;
  createdAt: string;
};

function Err({ value }: { value: string }) {
  if (!value) return null;
  return (
    <p className="mt-2 text-sm text-red-700 bg-red-50 border border-red-200 rounded px-3 py-2">
      {value}
    </p>
  );
}

function Ok({ value }: { value: string }) {
  if (!value) return null;
  return (
    <p className="mt-2 text-sm text-emerald-800 bg-emerald-50 border border-emerald-200 rounded px-3 py-2 whitespace-pre-wrap">
      {value}
    </p>
  );
}

export default function ProofPage() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [infoError, setInfoError] = useState("");
  const [pdf, setPdf] = useState<PdfResult | null>(null);
  const [pdfBusy, setPdfBusy] = useState(false);
  const [pdfError, setPdfError] = useState("");
  const [image, setImage] = useState<ImportResult | null>(null);
  const [imageBusy, setImageBusy] = useState(false);
  const [imageError, setImageError] = useState("");
  const [backup, setBackup] = useState<BackupResult | null>(null);
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupError, setBackupError] = useState("");
  const [backups, setBackups] = useState<BackupEntry[]>([]);

  const handleInfo = async () => {
    try {
      const result = await runCommand<AppInfo>("proof_app_info");
      setInfo(result);
      setInfoError("");
    } catch (e) {
      setInfoError(`${(e as CommandError).code}: ${(e as CommandError).message}`);
    }
  };

  const handlePdf = async () => {
    setPdfBusy(true);
    setPdfError("");
    try {
      const result = await runCommand<PdfResult>("proof_generate_pdf");
      setPdf(result);
    } catch (e) {
      setPdfError(`${(e as CommandError).code}: ${(e as CommandError).message}`);
    } finally {
      setPdfBusy(false);
    }
  };

  const handlePickImage = async () => {
    setImageBusy(true);
    setImageError("");
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: "Images", extensions: ["jpg", "jpeg", "png", "webp"] },
        ],
      });
      if (!selected || Array.isArray(selected)) {
        setImageBusy(false);
        return;
      }
      const result = await runCommand<ImportResult>("proof_import_image", {
        path: selected,
      });
      setImage(result);
    } catch (e) {
      setImageError(`${(e as CommandError).code}: ${(e as CommandError).message}`);
    } finally {
      setImageBusy(false);
    }
  };

  const handleBackup = async () => {
    setBackupBusy(true);
    setBackupError("");
    try {
      const result = await runCommand<BackupResult>("proof_create_backup");
      setBackup(result);
      const list = await runCommand<BackupEntry[]>("proof_list_backups");
      setBackups(list);
    } catch (e) {
      setBackupError(`${(e as CommandError).code}: ${(e as CommandError).message}`);
    } finally {
      setBackupBusy(false);
    }
  };

  const handleList = async () => {
    try {
      const list = await runCommand<BackupEntry[]>("proof_list_backups");
      setBackups(list);
      setBackupError("");
    } catch (e) {
      setBackupError(`${(e as CommandError).code}: ${(e as CommandError).message}`);
    }
  };

  return (
    <main className="mx-auto max-w-4xl px-6 py-10">
      <header className="mb-8">
        <p className="text-sm text-amber-accent font-semibold">
          Furniture Shop — Phase 0
        </p>
        <h1 className="text-2xl font-semibold text-forest-700">
          Technical Proof
        </h1>
        <p className="mt-1 text-sm text-neutral-500">
          Each panel exercises one of the Phase 0 prototypes: typed commands,
          SQLite migrations, image import, Unicode PDF, and WAL-safe backup.
        </p>
      </header>

      <section className="rounded-lg border bg-white p-5 shadow-sm">
        <h2 className="text-lg font-semibold">
          Typed command + SQLite migration
        </h2>
        <p className="text-sm text-neutral-500">
          Invokes a typed Tauri command into Rust, which reports app identity
          and the applied database schema version.
        </p>
        <div className="mt-3 flex gap-2">
          <button
            onClick={handleInfo}
            className="rounded-md bg-forest-600 px-4 py-2 text-sm font-medium text-white hover:bg-forest-700"
          >
            Check app &amp; database
          </button>
        </div>
        {info && (
          <pre className="mt-3 rounded bg-neutral-50 border border-neutral-200 p-3 text-xs overflow-x-auto">
            {JSON.stringify(info, null, 2)}
          </pre>
        )}
        <Err value={infoError} />
      </section>

      <section className="mt-5 rounded-lg border bg-white p-5 shadow-sm">
        <h2 className="text-lg font-semibold">Unicode PDF (English + Urdu)</h2>
        <p className="text-sm text-neutral-500">
          Generates a multi-line A4 PDF with PKR amounts and sample Urdu text
          using an embedded Unicode font.
        </p>
        <div className="mt-3 flex gap-2">
          <button
            onClick={handlePdf}
            disabled={pdfBusy}
            className="rounded-md bg-forest-600 px-4 py-2 text-sm font-medium text-white hover:bg-forest-700 disabled:opacity-50"
          >
            {pdfBusy ? "Generating…" : "Generate PDF"}
          </button>
          {pdf && !pdfBusy && (
            <button
              onClick={async () => {
                await runCommand("proof_open_path", { path: pdf.reportPath });
              }}
              className="rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium hover:bg-neutral-50"
            >
              Open
            </button>
          )}
        </div>
        {pdf && <Ok value={`Saved: ${pdf.reportPath}\nPages: ${pdf.pages}, Bytes: ${pdf.bytes.toLocaleString()}`} />}
        <Err value={pdfError} />
      </section>

      <section className="mt-5 rounded-lg border bg-white p-5 shadow-sm">
        <h2 className="text-lg font-semibold">Image import pipeline</h2>
        <p className="text-sm text-neutral-500">
          Picks an image, validates its decoded type, re-encodes, generates a
          thumbnail, and stores it in the app-data directory.
        </p>
        <div className="mt-3 flex items-center gap-3">
          <button
            onClick={handlePickImage}
            disabled={imageBusy}
            className="rounded-md bg-forest-600 px-4 py-2 text-sm font-medium text-white hover:bg-forest-700 disabled:opacity-50"
          >
            {imageBusy ? "Importing…" : "Pick image"}
          </button>
          {image && (
            <span className="text-xs text-neutral-500">
              Stored as <span className="tabular">{image.storedName}</span> ·
              {image.width}×{image.height} · thumbnail {image.thumbnailBytes} bytes
            </span>
          )}
        </div>
        <Err value={imageError} />
      </section>

      <section className="mt-5 rounded-lg border bg-white p-5 shadow-sm">
        <h2 className="text-lg font-semibold">WAL-safe backup</h2>
        <p className="text-sm text-neutral-500">
          Takes a consistent SQLite snapshot via the backup API, records its
          SHA-256, and verifies integrity. Works while WAL mode is active.
        </p>
        <div className="mt-3 flex gap-2">
          <button
            onClick={handleBackup}
            disabled={backupBusy}
            className="rounded-md bg-forest-600 px-4 py-2 text-sm font-medium text-white hover:bg-forest-700 disabled:opacity-50"
          >
            {backupBusy ? "Backing up…" : "Back up now"}
          </button>
          <button
            onClick={handleList}
            className="rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium hover:bg-neutral-50"
          >
            List backups
          </button>
        </div>
        {backup && (
          <Ok
            value={`Saved: ${backup.backupPath}\nSHA-256: ${backup.sha256} | verified: ${backup.verified}`
            }
          />
        )}
        {backups.length > 0 && (
          <table className="mt-3 w-full text-left text-xs tabular">
            <thead>
              <tr className="border-b text-neutral-500">
                <th className="py-1">Name</th>
                <th className="py-1">Bytes</th>
                <th className="py-1">SHA-256</th>
                <th className="py-1">Created</th>
              </tr>
            </thead>
            <tbody>
              {backups.map((b) => (
                <tr key={b.name} className="border-b border-neutral-100">
                  <td className="py-1">{b.name}</td>
                  <td className="py-1">{b.sizeBytes.toLocaleString()}</td>
                  <td className="py-1 font-mono">{b.sha256.slice(0, 16)}…</td>
                  <td className="py-1">{b.createdAt}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <Err value={backupError} />
      </section>
    </main>
  );
}