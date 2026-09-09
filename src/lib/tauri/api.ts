import { runCommand } from "@/lib/tauri/client";

export type SessionProfile = {
  sessionId: string;
  userId: number;
  username: string;
  fullName: string;
  roles: string[];
  permissions: string[];
  lockedAt?: string | null;
};

export type FirstRunStatus = {
  required: boolean;
  complete: boolean;
  hasUsers: boolean;
};

export type LoginResult = {
  sessionId: string;
  profile: SessionProfile;
};

export type UserDto = {
  id: number;
  username: string;
  fullName: string;
  isActive: boolean;
  roles: string[];
  createdAt: string;
};

export type RoleDto = {
  id: number;
  code: string;
  name: string;
  description?: string | null;
  isSystem: boolean;
  permissions: string[];
};

export type AuditEvent = {
  id: number;
  userId?: number | null;
  action: string;
  entityType?: string | null;
  entityId?: string | null;
  reason?: string | null;
  beforeJson?: string | null;
  afterJson?: string | null;
  approvalUserId?: number | null;
  sessionId?: string | null;
  appVersion?: string | null;
  correlationId?: string | null;
  prevHash?: string | null;
  createdAt: string;
};

export type AuditPage = {
  total: number;
  items: AuditEvent[];
};

export type AppInfo = {
  appName: string;
  version: string;
  dataDir: string;
  dbPath: string;
  dbVersion: number;
  pendingMigrations: number;
};

// --- First-run setup ---------------------------------------------------------

export type FirstRunCompleteInput = {
  shopName: string;
  shopAddress: string;
  shopPhone: string;
  shopEmail: string;
  currency: string;
  timezone: string;
  firstLocation: string;
  invoicePrefix?: string | null;
  backupLocation?: string | null;
  ownerUsername: string;
  ownerFullName: string;
  ownerPassword: string;
};

export const firstRunStatus = () => runCommand<FirstRunStatus>("first_run_status");

export const firstRunComplete = (input: FirstRunCompleteInput) =>
  runCommand<LoginResult>("first_run_complete", {
    shopName: input.shopName,
    shopAddress: input.shopAddress,
    shopPhone: input.shopPhone,
    shopEmail: input.shopEmail,
    currency: input.currency,
    timezone: input.timezone,
    firstLocation: input.firstLocation,
    invoicePrefix: input.invoicePrefix || null,
    backupLocation: input.backupLocation || null,
    ownerUsername: input.ownerUsername,
    ownerFullName: input.ownerFullName,
    ownerPassword: input.ownerPassword,
  });

// --- Authentication ----------------------------------------------------------

export const authLogin = (username: string, password: string) =>
  runCommand<LoginResult>("auth_login", { username, password });

export const authLogout = (session: string) =>
  runCommand<void>("auth_logout", { session });

export const authCurrent = (session: string) =>
  runCommand<SessionProfile | null>("auth_current", { session });

export const authLock = (session: string) => runCommand<void>("auth_lock", { session });

export const authUnlock = (session: string, password: string) =>
  runCommand<SessionProfile>("auth_unlock", { session, password });

export const authChangePassword = (
  session: string,
  currentPassword: string,
  newPassword: string,
) =>
  runCommand<void>("auth_change_password", {
    session,
    currentPassword,
    newPassword,
  });

// --- Users -------------------------------------------------------------------

export const userList = (session: string) =>
  runCommand<UserDto[]>("user_list", { session });

export const userCreate = (
  session: string,
  input: { username: string; fullName: string; password: string; roles: string[] },
) =>
  runCommand<UserDto>("user_create", {
    session,
    username: input.username,
    fullName: input.fullName,
    password: input.password,
    roles: input.roles,
  });

export const userUpdate = (
  session: string,
  input: {
    userId: number;
    fullName?: string | null;
    isActive?: boolean | null;
    roles?: string[] | null;
    newPassword?: string | null;
  },
) =>
  runCommand<UserDto>("user_update", {
    session,
    userId: input.userId,
    fullName: input.fullName ?? null,
    isActive: input.isActive ?? null,
    roles: input.roles ?? null,
    newPassword: input.newPassword ?? null,
  });

export const userDeactivate = (session: string, userId: number, reason: string) =>
  runCommand<void>("user_deactivate", { session, userId, reason });

export const userResetPassword = (session: string, userId: number, newPassword: string) =>
  runCommand<void>("user_reset_password", { session, userId, newPassword });

// --- Roles -------------------------------------------------------------------

export const roleList = (session: string) => runCommand<RoleDto[]>("role_list", { session });

export const rolePermissionsSet = (
  session: string,
  roleId: number,
  permissions: string[],
) => runCommand<void>("role_permissions_set", { session, roleId, permissions });

// --- Audit -------------------------------------------------------------------

export type AuditFilter = {
  action?: string | null;
  entityType?: string | null;
  userId?: number | null;
  from?: string | null;
  to?: string | null;
  limit?: number;
  offset?: number;
};

export const auditQuery = (session: string, filter: AuditFilter) =>
  runCommand<AuditPage>("audit_query", {
    session,
    action: filter.action || null,
    entityType: filter.entityType || null,
    userId: filter.userId ?? null,
    from: filter.from || null,
    to: filter.to || null,
    limit: filter.limit ?? 50,
    offset: filter.offset ?? 0,
  });

// --- Settings ----------------------------------------------------------------

export const settingsGet = (session: string, key: string) =>
  runCommand<string | null>("settings_get", { session, key });

// --- Proof / system (Phase 1 dashboard) --------------------------------------

export const proofAppInfo = () => runCommand<AppInfo>("proof_app_info");

export type PdfResult = {
  reportPath: string;
  pages: number;
  bytes: number;
};

export const proofGeneratePdf = () => runCommand<PdfResult>("proof_generate_pdf");

export type ImportResult = {
  originalPath: string;
  storedName: string;
  width: number;
  height: number;
  thumbnailBytes: number;
};

export const proofImportImage = (path: string) =>
  runCommand<ImportResult>("proof_import_image", { path });

export type BackupResult = {
  backupPath: string;
  sha256: string;
  verified: boolean;
  bytes: number;
};

export type BackupEntry = {
  name: string;
  sizeBytes: number;
  sha256: string;
  createdAt: string;
};

export const proofCreateBackup = () => runCommand<BackupResult>("proof_create_backup");

export const proofListBackups = () => runCommand<BackupEntry[]>("proof_list_backups");

export const proofOpenPath = (path: string) => runCommand<void>("proof_open_path", { path });