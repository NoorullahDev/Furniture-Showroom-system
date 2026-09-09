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
  entityId?: string | null;
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
    entityId: filter.entityId || null,
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

// --- Catalogue: categories, product types, units ------------------------------

export type CategoryDto = {
  id: number;
  name: string;
  parentId: number | null;
  sortOrder: number;
  isActive: boolean;
  productCount: number;
};

export type ProductTypeDto = {
  id: number;
  categoryId: number;
  name: string;
  isActive: boolean;
  productCount: number;
};

export type UnitDto = {
  id: number;
  name: string;
  code?: string | null;
};

export const categoryList = (session: string) =>
  runCommand<CategoryDto[]>("category_list", { session });

export const categoryCreate = (
  session: string,
  input: { name: string; parentId?: number | null; sortOrder?: number | null },
) =>
  runCommand<CategoryDto>("category_create", {
    session,
    name: input.name,
    parentId: input.parentId ?? null,
    sortOrder: input.sortOrder ?? null,
  });

export const categoryUpdate = (
  session: string,
  input: {
    categoryId: number;
    name?: string | null;
    parentId?: number | null;
    sortOrder?: number | null;
    isActive?: boolean | null;
  },
) =>
  runCommand<CategoryDto>("category_update", {
    session,
    categoryId: input.categoryId,
    name: input.name ?? null,
    parentId: input.parentId === undefined ? null : input.parentId,
    sortOrder: input.sortOrder ?? null,
    isActive: input.isActive ?? null,
  });

export const categoryArchive = (session: string, categoryId: number, reason?: string) =>
  runCommand<void>("category_archive", {
    session,
    categoryId,
    reason: reason || null,
  });

export const productTypeList = (session: string, categoryId?: number | null) =>
  runCommand<ProductTypeDto[]>("product_type_list", {
    session,
    categoryId: categoryId ?? null,
  });

export const productTypeCreate = (session: string, categoryId: number, name: string) =>
  runCommand<ProductTypeDto>("product_type_create", {
    session,
    categoryId,
    name,
  });

export const productTypeUpdate = (
  session: string,
  input: { productTypeId: number; name?: string | null; isActive?: boolean | null },
) =>
  runCommand<ProductTypeDto>("product_type_update", {
    session,
    productTypeId: input.productTypeId,
    name: input.name ?? null,
    isActive: input.isActive ?? null,
  });

export const productTypeArchive = (session: string, productTypeId: number, reason?: string) =>
  runCommand<void>("product_type_archive", {
    session,
    productTypeId,
    reason: reason || null,
  });

export const unitList = (session: string) => runCommand<UnitDto[]>("unit_list", { session });

// --- Catalogue: products ------------------------------------------------------

export type AttributeInputDto = {
  name: string;
  value: string;
};

export type ProductImageDto = {
  id: number;
  imagePath: string;
  thumbnailPath: string;
  relativePath: string;
  thumbnailRelativePath: string;
  sortOrder: number;
  isPrimary: boolean;
  sha256: string;
  width?: number | null;
  height?: number | null;
  mimeType: string;
};

export type ProductListItemDto = {
  id: number;
  articleNumber: string;
  name: string;
  categoryId: number;
  category: string;
  productTypeId: number | null;
  productType: string | null;
  unit: string | null;
  salePriceMinor: number;
  costMinor: number | null;
  minimumStock: number;
  trackStock: boolean;
  isActive: boolean;
  archivedAt: string | null;
  primaryThumbnailPath: string | null;
};

export type ProductDetailDto = {
  id: number;
  articleNumber: string;
  name: string;
  categoryId: number;
  category: string;
  productTypeId: number | null;
  productType: string | null;
  unitId: number | null;
  unit: string | null;
  description: string | null;
  material: string | null;
  color: string | null;
  dimensionsText: string | null;
  brand: string | null;
  barcode: string | null;
  warrantyMonths: number | null;
  notes: string | null;
  costMinor: number | null;
  salePriceMinor: number;
  minimumStock: number;
  trackStock: boolean;
  isActive: boolean;
  archivedAt: string | null;
  createdAt: string;
  updatedAt: string;
  images: ProductImageDto[];
  attributes: AttributeInputDto[];
};

export type CreateProductInput = {
  articleNumber: string;
  name: string;
  categoryId: number;
  productTypeId?: number | null;
  unitId?: number | null;
  description?: string | null;
  material?: string | null;
  color?: string | null;
  dimensionsText?: string | null;
  brand?: string | null;
  barcode?: string | null;
  warrantyMonths?: number | null;
  notes?: string | null;
  costMinor: number;
  salePriceMinor: number;
  minimumStock: number;
  trackStock: boolean;
  attributes: AttributeInputDto[];
  imagePaths: string[];
};

export type UpdateProductInput = {
  articleNumber?: string | null;
  name?: string | null;
  categoryId?: number | null;
  productTypeId?: number | null;
  unitId?: number | null;
  description?: string | null;
  material?: string | null;
  color?: string | null;
  dimensionsText?: string | null;
  brand?: string | null;
  barcode?: string | null;
  warrantyMonths?: number | null;
  notes?: string | null;
  costMinor?: number | null;
  salePriceMinor?: number | null;
  minimumStock?: number | null;
  trackStock?: boolean | null;
  attributes?: AttributeInputDto[] | null;
};

export type ProductListFilter = {
  scope?: "active" | "archived" | "all";
  q?: string;
  categoryId?: number | null;
  productTypeId?: number | null;
  priceMin?: number | null;
  priceMax?: number | null;
  attributeQ?: string | null;
};

export const productList = (session: string, filter?: ProductListFilter) =>
  runCommand<ProductListItemDto[]>("product_list", {
    session,
    scope: filter?.scope ?? "active",
    q: filter?.q || null,
    categoryId: filter?.categoryId ?? null,
    productTypeId: filter?.productTypeId ?? null,
    priceMin: filter?.priceMin ?? null,
    priceMax: filter?.priceMax ?? null,
    attributeQ: filter?.attributeQ || null,
  });

export const productGet = (session: string, productId: number) =>
  runCommand<ProductDetailDto>("product_get", { session, productId });

export const productCreate = (session: string, input: CreateProductInput) =>
  runCommand<ProductDetailDto>("product_create", { session, input });

export const productUpdate = (session: string, productId: number, input: UpdateProductInput) =>
  runCommand<ProductDetailDto>("product_update", { session, productId, input });

export const productArchive = (session: string, productId: number, reason?: string) =>
  runCommand<void>("product_archive", {
    session,
    productId,
    reason: reason || null,
  });

export const productUnarchive = (session: string, productId: number) =>
  runCommand<ProductDetailDto>("product_unarchive", { session, productId });

export const productDuplicate = (session: string, productId: number, newArticle: string) =>
  runCommand<ProductDetailDto>("product_duplicate", { session, productId, newArticle });

export const productImageAdd = (session: string, productId: number, path: string) =>
  runCommand<ProductDetailDto>("product_image_add", { session, productId, path });

export const productImageRemove = (session: string, productId: number, imageId: number) =>
  runCommand<ProductDetailDto>("product_image_remove", { session, productId, imageId });

export const productImageSetPrimary = (
  session: string,
  productId: number,
  imageId: number,
) => runCommand<ProductDetailDto>("product_image_set_primary", { session, productId, imageId });

export const productImageReorder = (
  session: string,
  productId: number,
  orderedIds: number[],
) => runCommand<ProductDetailDto>("product_image_reorder", { session, productId, orderedIds });

/** Read a stored product image and return it as a `data:` URL for `<img>` use. */
export const productImageData = (session: string, path: string) =>
  runCommand<string>("product_image_data", { session, path });

// --- Inventory (Phase 4) ------------------------------------------------------

export type LocationDto = {
  id: number;
  name: string;
  locationType: string;
  isActive: boolean;
};

export type StockBalanceDto = {
  productId: number;
  articleNumber: string;
  productName: string;
  thumbnailPath: string | null;
  locationId: number;
  locationName: string;
  onHand: number;
  reserved: number;
  damaged: number;
  minimumStock: number;
  available: number;
};

export type StockMovementDto = {
  id: number;
  productId: number;
  articleNumber: string | null;
  productName: string | null;
  locationId: number;
  locationName: string | null;
  movementType: string;
  quantityDelta: number;
  moveNumber: string | null;
  unitCostMinor: number | null;
  referenceType: string | null;
  referenceId: number | null;
  reason: string | null;
  reversalOfId: number | null;
  createdBy: number;
  createdAt: string;
};

export type ValuationLineDto = {
  productId: number;
  articleNumber: string;
  productName: string;
  unitCostMinor: number;
  sellableQty: number;
  valueMinor: number;
};

export type PostStockInput = {
  productId: number;
  locationId: number;
  quantity: number;
  unitCostMinor?: number | null;
  reason?: string | null;
};

export type TransferStockInput = {
  productId: number;
  fromLocationId: number;
  toLocationId: number;
  quantity: number;
  reason?: string | null;
};

export type AdjustStockInput = {
  productId: number;
  locationId: number;
  adjustmentQty: number;
  unitCostMinor?: number | null;
  reason?: string | null;
};

export type DamageStockInput = {
  productId: number;
  locationId: number;
  quantity: number;
  reason?: string | null;
};

export type ReleaseStockInput = {
  productId: number;
  locationId: number;
  quantity: number;
  reason?: string | null;
};

export const locationList = (session: string) =>
  runCommand<LocationDto[]>("location_list", { session });

export const stockBalanceList = (session: string, locationId?: number | null) =>
  runCommand<StockBalanceDto[]>("stock_balance_list", { session, locationId: locationId ?? null });

export const stockMovementList = (
  session: string,
  opts?: { productId?: number | null; locationId?: number | null; limit?: number | null },
) =>
  runCommand<StockMovementDto[]>("stock_movement_list", {
    session,
    productId: opts?.productId ?? null,
    locationId: opts?.locationId ?? null,
    limit: opts?.limit ?? null,
  });

export const stockValuation = (session: string) =>
  runCommand<ValuationLineDto[]>("stock_valuation", { session });

export const stockOpening = (session: string, input: PostStockInput) =>
  runCommand<StockMovementDto>("stock_opening", { session, input });

export const stockTransfer = (session: string, input: TransferStockInput) =>
  runCommand<StockMovementDto[]>("stock_transfer", { session, input });

export const stockAdjust = (session: string, input: AdjustStockInput) =>
  runCommand<StockMovementDto>("stock_adjust", { session, input });

export const stockDamage = (session: string, input: DamageStockInput) =>
  runCommand<StockMovementDto>("stock_damage", { session, input });

export const stockRepair = (session: string, input: DamageStockInput) =>
  runCommand<StockMovementDto>("stock_repair", { session, input });

export const stockReserve = (session: string, input: ReleaseStockInput) =>
  runCommand<StockMovementDto>("stock_reserve", { session, input });

export const stockRelease = (session: string, input: ReleaseStockInput) =>
  runCommand<StockMovementDto>("stock_release", { session, input });

// --- Phase 4 additions: reversal, low-stock, count sessions, batch import ----

export type ReverseMovementInput = {
  movementId: number;
  reason?: string | null;
};

export type LowStockItemDto = {
  productId: number;
  articleNumber: string;
  productName: string;
  thumbnailPath: string | null;
  minimumStock: number;
  totalAvailable: number;
  totalOnHand: number;
};

export type CountSessionDto = {
  id: number;
  locationId: number;
  locationName: string | null;
  sessionNumber: string | null;
  status: string;
  notes: string | null;
  createdBy: number;
  createdAt: string;
  postedAt: string | null;
};

export type CountLineDto = {
  id: number;
  sessionId: number;
  productId: number;
  articleNumber: string | null;
  productName: string | null;
  expectedQty: number;
  countedQty: number;
  varianceQty: number;
};

export type StartCountInput = {
  locationId: number;
  notes?: string | null;
};

export type CountLineInput = {
  sessionId: number;
  productId: number;
  countedQty: number;
};

export type PostCountInput = {
  sessionId: number;
};

export type OpeningBatchRowInput = {
  articleNumber: string;
  locationId: number;
  quantity: number;
  unitCostMinor?: number | null;
  reason?: string | null;
};

export type OpeningBatchInput = {
  rows: OpeningBatchRowInput[];
};

export type OpeningBatchErrorDto = {
  rowIndex: number;
  articleNumber: string;
  error: string;
};

export type OpeningBatchResultDto = {
  postedCount: number;
  errorCount: number;
  errors: OpeningBatchErrorDto[];
};

export const stockReverse = (session: string, input: ReverseMovementInput) =>
  runCommand<StockMovementDto>("stock_reverse", { session, input });

export const stockLowList = (session: string) =>
  runCommand<LowStockItemDto[]>("stock_low_list", { session });

export const stockCountStart = (session: string, input: StartCountInput) =>
  runCommand<CountSessionDto>("stock_count_start", { session, input });

export const stockCountLineUpdate = (session: string, input: CountLineInput) =>
  runCommand<CountLineDto>("stock_count_line_update", { session, input });

export const stockCountLines = (session: string, sessionId: number) =>
  runCommand<CountLineDto[]>("stock_count_lines", { session, sessionId });

export const stockCountPost = (session: string, input: PostCountInput) =>
  runCommand<StockMovementDto[]>("stock_count_post", { session, input });

export const stockCountList = (session: string, locationId?: number | null) =>
  runCommand<CountSessionDto[]>("stock_count_list", { session, locationId: locationId ?? null });

export const stockOpeningBatch = (session: string, input: OpeningBatchInput) =>
  runCommand<OpeningBatchResultDto>("stock_opening_batch", { session, input });