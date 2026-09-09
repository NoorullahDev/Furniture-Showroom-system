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

// ---------------------------------------------------------------------------
// Phase 5 — Suppliers, purchases, payables, and cash
// ---------------------------------------------------------------------------

export type SupplierDto = {
  id: number;
  code: string;
  name: string;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  openingBalanceMinor: number;
  balanceMinor: number;
  isActive: boolean;
  createdAt: string;
};

export type SupplierInput = {
  code: string;
  name: string;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  openingBalanceMinor?: number | null;
  isActive?: boolean | null;
};

export type SupplierLedgerEntryDto = {
  id: number;
  entryType: string;
  documentType?: string | null;
  documentId?: number | null;
  amountMinor: number;
  balanceAfterMinor: number;
  notes?: string | null;
  createdBy: number;
  createdAt: string;
};

export type PaymentMethodDto = {
  id: number;
  code: string;
  name: string;
  isActive: boolean;
};

export type CashAccountDto = {
  id: number;
  code: string;
  name: string;
  kind: string;
  openingBalanceMinor: number;
  balanceMinor: number;
  isActive: boolean;
};

export type CashAccountInput = {
  code: string;
  name: string;
  kind?: string | null;
  openingBalanceMinor?: number | null;
};

export type CashEntryDto = {
  id: number;
  cashAccountId: number;
  entryType: string;
  amountMinor: number;
  referenceType?: string | null;
  referenceId?: number | null;
  reason?: string | null;
  createdBy: number;
  createdAt: string;
};

export type PurchaseItemInput = {
  productId: number;
  quantity: number;
  unitCostMinor: number;
};

export type PurchaseCreateInput = {
  supplierId: number;
  locationId: number;
  invoiceNumber: string;
  invoiceDate: string;
  purchaseDate?: string | null;
  notes?: string | null;
  items: PurchaseItemInput[];
};

export type PurchasePostInput = {
  purchaseId: number;
  idempotencyKey?: string | null;
  paidMinor?: number | null;
  cashAccountId?: number | null;
  paymentMethodId?: number | null;
};

export type PurchaseItemDto = {
  productId: number;
  articleNumber: string;
  productName: string;
  quantity: number;
  unitCostMinor: number;
  lineTotalMinor: number;
};

export type PurchaseDto = {
  id: number;
  purchaseNumber?: string | null;
  supplierId: number;
  supplierName: string;
  locationId: number;
  invoiceNumber: string;
  invoiceDate: string;
  purchaseDate: string;
  status: string;
  totalMinor: number;
  paidMinor: number;
  dueMinor: number;
  notes?: string | null;
  items: PurchaseItemDto[];
  createdAt: string;
  postedAt?: string | null;
};

export type PayableAgingRowDto = {
  purchaseId: number;
  purchaseNumber?: string | null;
  supplierId: number;
  supplierName: string;
  invoiceDate: string;
  dueMinor: number;
  ageDays: number;
  bucket: string;
};

export type SupplierPaymentInput = {
  supplierId: number;
  paymentMethodId: number;
  cashAccountId: number;
  paymentDate: string;
  amountMinor: number;
  notes?: string | null;
  idempotencyKey?: string | null;
};

export type PaymentAllocationDto = {
  purchaseId: number;
  purchaseNumber?: string | null;
  amountMinor: number;
};

export type SupplierPaymentDto = {
  id: number;
  paymentNumber?: string | null;
  supplierId: number;
  supplierName: string;
  paymentMethodId: number;
  paymentMethodName: string;
  cashAccountId: number;
  cashAccountName: string;
  paymentDate: string;
  amountMinor: number;
  status: string;
  notes?: string | null;
  allocations: PaymentAllocationDto[];
  createdAt: string;
  voidedAt?: string | null;
};

export type SupplierPaymentVoidInput = {
  paymentId: number;
  reason?: string | null;
};

export type SupplierReturnItemInput = {
  productId: number;
  quantity: number;
  unitCostMinor: number;
};

export type SupplierReturnCreateInput = {
  supplierId: number;
  purchaseId?: number | null;
  locationId: number;
  returnDate: string;
  refundMinor?: number | null;
  notes?: string | null;
  items: SupplierReturnItemInput[];
};

export type SupplierReturnPostInput = {
  returnId: number;
  idempotencyKey?: string | null;
};

export type SupplierReturnItemDto = {
  productId: number;
  articleNumber: string;
  productName: string;
  quantity: number;
  unitCostMinor: number;
  lineTotalMinor: number;
};

export type SupplierReturnDto = {
  id: number;
  returnNumber?: string | null;
  supplierId: number;
  supplierName: string;
  purchaseId?: number | null;
  locationId: number;
  returnDate: string;
  status: string;
  totalMinor: number;
  refundMinor: number;
  dueReductionMinor: number;
  notes?: string | null;
  items: SupplierReturnItemDto[];
  createdAt: string;
  postedAt?: string | null;
};

export const supplierList = (session: string) =>
  runCommand<SupplierDto[]>("supplier_list", { session });

export const supplierCreate = (session: string, input: SupplierInput) =>
  runCommand<SupplierDto>("supplier_create", { session, input });

export const supplierUpdate = (session: string, supplierId: number, input: SupplierInput) =>
  runCommand<SupplierDto>("supplier_update", { session, supplierId, input });

export const supplierGet = (session: string, supplierId: number) =>
  runCommand<SupplierDto>("supplier_get", { session, supplierId });

export const supplierLedger = (session: string, supplierId: number) =>
  runCommand<SupplierLedgerEntryDto[]>("supplier_ledger", { session, supplierId });

export const paymentMethodList = (session: string) =>
  runCommand<PaymentMethodDto[]>("payment_method_list", { session });

export const cashAccountList = (session: string) =>
  runCommand<CashAccountDto[]>("cash_account_list", { session });

export const cashAccountCreate = (session: string, input: CashAccountInput) =>
  runCommand<CashAccountDto>("cash_account_create", { session, input });

export const cashEntryList = (session: string, accountId?: number | null, limit?: number | null) =>
  runCommand<CashEntryDto[]>("cash_entry_list", {
    session,
    accountId: accountId ?? null,
    limit: limit ?? null,
  });

export const purchaseCreate = (session: string, input: PurchaseCreateInput) =>
  runCommand<PurchaseDto>("purchase_create", { session, input });

export const purchasePost = (session: string, input: PurchasePostInput) =>
  runCommand<PurchaseDto>("purchase_post", { session, input });

export const purchaseList = (session: string) =>
  runCommand<PurchaseDto[]>("purchase_list", { session });

export const purchaseGet = (session: string, purchaseId: number) =>
  runCommand<PurchaseDto>("purchase_get", { session, purchaseId });

export const payableAging = (session: string) =>
  runCommand<PayableAgingRowDto[]>("payable_aging", { session });

export const supplierPaymentCreate = (session: string, input: SupplierPaymentInput) =>
  runCommand<SupplierPaymentDto>("supplier_payment_create", { session, input });

export const supplierPaymentVoid = (session: string, input: SupplierPaymentVoidInput) =>
  runCommand<SupplierPaymentDto>("supplier_payment_void", { session, input });

export const supplierPaymentList = (session: string) =>
  runCommand<SupplierPaymentDto[]>("supplier_payment_list", { session });

export const supplierReturnCreate = (session: string, input: SupplierReturnCreateInput) =>
  runCommand<SupplierReturnDto>("supplier_return_create", { session, input });

export const supplierReturnPost = (session: string, input: SupplierReturnPostInput) =>
  runCommand<SupplierReturnDto>("supplier_return_post", { session, input });

export const supplierReturnList = (session: string) =>
  runCommand<SupplierReturnDto[]>("supplier_return_list", { session });

// ---------------------------------------------------------------------------
// Phase 6 — Customers, furniture sets (bundles), sales, receipts, invoices
// ---------------------------------------------------------------------------

export type CustomerDto = {
  id: number;
  code: string;
  name: string;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  creditLimitMinor: number;
  creditDays: number;
  openingBalanceMinor: number;
  balanceMinor: number;
  advanceMinor: number;
  isActive: boolean;
  createdAt: string;
};

export type CustomerInput = {
  code: string;
  name: string;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  creditLimitMinor?: number | null;
  creditDays?: number | null;
  openingBalanceMinor?: number | null;
  isActive?: boolean | null;
};

export type CustomerLedgerEntryDto = {
  id: number;
  entryType: string;
  documentType?: string | null;
  documentId?: number | null;
  amountMinor: number;
  balanceAfterMinor: number;
  notes?: string | null;
  createdBy: number;
  createdAt: string;
};

export type BundleItemInput = {
  productId: number;
  quantity: number;
};

export type BundleInput = {
  code: string;
  name: string;
  description?: string | null;
  coverImagePath?: string | null;
  defaultPriceMinor?: number | null;
  isActive?: boolean | null;
  items: BundleItemInput[];
};

export type BundleItemDto = {
  productId: number;
  articleNumber: string;
  productName: string;
  quantity: number;
  sortOrder: number;
  unitCostEstimateMinor: number;
  lineCostEstimateMinor: number;
};

export type BundleDto = {
  id: number;
  code: string;
  name: string;
  description?: string | null;
  coverImagePath?: string | null;
  defaultPriceMinor: number;
  isActive: boolean;
  items: BundleItemDto[];
  costEstimateMinor: number;
  createdAt: string;
};

export type BundleAvailabilityDto = {
  bundleId: number;
  bundleName: string;
  locationId: number;
  availableCount: number;
  limitingProductId?: number | null;
  limitingProductName?: string | null;
  limitingAvailable?: number | null;
  limitingNeededPerSet?: number | null;
};

export type SaleItemInput = {
  productId?: number | null;
  bundleId?: number | null;
  quantity: number;
};

export type SaleComponentDto = {
  productId: number;
  articleNumber: string;
  productName: string;
  quantity: number;
  unitCostMinor: number;
  lineCostMinor: number;
};

export type SaleItemDto = {
  id: number;
  productId?: number | null;
  bundleId?: number | null;
  articleNumber: string;
  productName: string;
  quantity: number;
  unitPriceMinor: number;
  lineTotalMinor: number;
  unitCostMinor: number;
  lineCostMinor: number;
  components: SaleComponentDto[];
};

export type SaleCreateInput = {
  locationId: number;
  customerId?: number | null;
  kind?: string | null;
  saleDate?: string | null;
  discountMinor?: number | null;
  deliveryChargeMinor?: number | null;
  notes?: string | null;
  items: SaleItemInput[];
};

export type SaleConfirmInput = {
  saleId: number;
  idempotencyKey?: string | null;
  paidMinor?: number | null;
  cashAccountId?: number | null;
  paymentMethodId?: number | null;
  advanceUsedMinor?: number | null;
};

export type SaleDto = {
  id: number;
  saleNumber?: string | null;
  kind: string;
  customerId?: number | null;
  customerName?: string | null;
  locationId: number;
  saleDate: string;
  status: string;
  subtotalMinor: number;
  discountMinor: number;
  deliveryChargeMinor: number;
  taxMinor: number;
  totalMinor: number;
  paidMinor: number;
  advanceUsedMinor: number;
  dueMinor: number;
  costMinor: number;
  notes?: string | null;
  items: SaleItemDto[];
  createdAt: string;
  confirmedAt?: string | null;
  confirmedBy?: number | null;
};

export type SaleCancelInput = {
  saleId: number;
  reason?: string | null;
};

export type CustomerReceiptAllocationInput = {
  saleId: number;
  amountMinor: number;
};

export type CustomerReceiptInput = {
  customerId: number;
  paymentMethodId: number;
  cashAccountId: number;
  paymentDate: string;
  amountMinor: number;
  notes?: string | null;
  idempotencyKey?: string | null;
  allocations?: CustomerReceiptAllocationInput[] | null;
};

export type SalePaymentAllocationDto = {
  saleId: number;
  saleNumber?: string | null;
  amountMinor: number;
};

export type CustomerPaymentDto = {
  id: number;
  receiptNumber?: string | null;
  customerId: number;
  customerName: string;
  saleId?: number | null;
  paymentMethodId: number;
  paymentMethodName: string;
  cashAccountId: number;
  cashAccountName: string;
  paymentDate: string;
  amountMinor: number;
  advanceAllocMinor: number;
  status: string;
  notes?: string | null;
  allocations: SalePaymentAllocationDto[];
  createdAt: string;
  voidedAt?: string | null;
};

export type CustomerPaymentVoidInput = {
  paymentId: number;
  reason?: string | null;
};

export const customerList = (session: string) =>
  runCommand<CustomerDto[]>("customer_list", { session });

export const customerGet = (session: string, customerId: number) =>
  runCommand<CustomerDto>("customer_get", { session, customerId });

export const customerCreate = (session: string, input: CustomerInput) =>
  runCommand<CustomerDto>("customer_create", { session, input });

export const customerUpdate = (
  session: string,
  customerId: number,
  input: CustomerInput,
) => runCommand<CustomerDto>("customer_update", { session, customerId, input });

export const customerLedger = (session: string, customerId: number) =>
  runCommand<CustomerLedgerEntryDto[]>("customer_ledger", { session, customerId });

export const customerReceiptCreate = (session: string, input: CustomerReceiptInput) =>
  runCommand<CustomerPaymentDto>("customer_receipt_create", { session, input });

export const customerReceiptVoid = (session: string, input: CustomerPaymentVoidInput) =>
  runCommand<CustomerPaymentDto>("customer_receipt_void", { session, input });

export const customerReceiptList = (session: string, customerId: number) =>
  runCommand<CustomerPaymentDto[]>("customer_receipt_list", { session, customerId });

export const bundleList = (session: string) =>
  runCommand<BundleDto[]>("bundle_list", { session });

export const bundleGet = (session: string, bundleId: number) =>
  runCommand<BundleDto>("bundle_get", { session, bundleId });

export const bundleCreate = (session: string, input: BundleInput) =>
  runCommand<BundleDto>("bundle_create", { session, input });

export const bundleUpdate = (session: string, bundleId: number, input: BundleInput) =>
  runCommand<BundleDto>("bundle_update", { session, bundleId, input });

export const bundleAvailability = (session: string, bundleId: number, locationId: number) =>
  runCommand<BundleAvailabilityDto>("bundle_availability", { session, bundleId, locationId });

export const saleCreate = (session: string, input: SaleCreateInput) =>
  runCommand<SaleDto>("sale_create", { session, input });

export const saleConfirm = (session: string, input: SaleConfirmInput) =>
  runCommand<SaleDto>("sale_confirm", { session, input });

export const saleCancel = (session: string, input: SaleCancelInput) =>
  runCommand<SaleDto>("sale_cancel", { session, input });

export const saleList = (session: string) =>
  runCommand<SaleDto[]>("sale_list", { session });

export const saleGet = (session: string, saleId: number) =>
  runCommand<SaleDto>("sale_get", { session, saleId });

export const saleInvoicePdf = (session: string, saleId: number) =>
  runCommand<PdfResult>("sale_invoice_pdf", { session, saleId });

// Phase 7 — Customer statements, receipt allocation preview, due control
// ---------------------------------------------------------------------------

export type CustomerStatementInput = {
  customerId: number;
  fromDate: string;
  toDate: string;
};

export type CustomerStatementDto = {
  customerId: number;
  customerName: string;
  fromDate: string;
  toDate: string;
  openingBalanceMinor: number;
  closingBalanceMinor: number;
  entries: CustomerLedgerEntryDto[];
};

export type CustomerReceiptPreviewInput = {
  customerId: number;
  amountMinor: number;
};

export type ReceiptAllocationPreviewDto = {
  saleId: number;
  saleNumber?: string | null;
  saleDate: string;
  dueDate?: string | null;
  totalMinor: number;
  dueMinor: number;
  allocatedMinor: number;
};

export type ReceiptPreviewDto = {
  customerId: number;
  customerName: string;
  amountMinor: number;
  allocations: ReceiptAllocationPreviewDto[];
  advanceMinor: number;
};

export type ReceivableSaleDto = {
  saleId: number;
  saleNumber?: string | null;
  customerId: number;
  customerName: string;
  saleDate: string;
  dueDate?: string | null;
  totalMinor: number;
  paidMinor: number;
  advanceUsedMinor: number;
  dueMinor: number;
  days: number;
};

export type ReceivableCustomerDto = {
  customerId: number;
  customerName: string;
  phone?: string | null;
  balanceMinor: number;
  creditLimitMinor: number;
  dueMinorTotal: number;
  overdueMinorTotal: number;
};

export type ReceivablesDto = {
  overdue: ReceivableSaleDto[];
  dueSoon: ReceivableSaleDto[];
  highBalance: ReceivableCustomerDto[];
  creditLimitExceptions: ReceivableCustomerDto[];
};

export const customerStatement = (session: string, input: CustomerStatementInput) =>
  runCommand<CustomerStatementDto>("customer_statement", { session, input });

export const customerReceiptPreview = (session: string, input: CustomerReceiptPreviewInput) =>
  runCommand<ReceiptPreviewDto>("customer_receipt_preview", { session, input });

export const customerReceiptPdf = (session: string, paymentId: number) =>
  runCommand<PdfResult>("customer_receipt_pdf", { session, paymentId });

export const receivables = (session: string) => runCommand<ReceivablesDto>("receivables", { session });