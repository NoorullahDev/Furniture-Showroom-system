export type PermissionDef = {
  code: string;
  label: string;
  description: string;
};

export const PERMISSION_GROUPS: { group: string; permissions: PermissionDef[] }[] = [
  {
    group: "Catalogue",
    permissions: [
      {
        code: "product.create",
        label: "Create products",
        description: "Add and edit catalogue items (requires Phase 3).",
      },
      {
        code: "product.cost.view",
        label: "View product cost",
        description: "See product cost and margin information.",
      },
    ],
  },
  {
    group: "Sales",
    permissions: [
      { code: "sale.create", label: "Create sales", description: "Enter quotations and sales." },
      {
        code: "sale.discount.override",
        label: "Override discount",
        description: "Apply discounts above the configured limit.",
      },
      { code: "sale.cancel", label: "Cancel sales", description: "Cancel confirmed sales." },
    ],
  },
  {
    group: "Payments",
    permissions: [
      { code: "payment.void", label: "Void payments", description: "Reverse posted payments." },
      { code: "supplier.pay", label: "Pay suppliers", description: "Record supplier payments." },
    ],
  },
  {
    group: "Purchasing",
    permissions: [
      {
        code: "supplier.create",
        label: "Create suppliers",
        description: "Add and edit supplier profiles.",
      },
      {
        code: "purchase.create",
        label: "Create purchases",
        description: "Record and post supplier purchases.",
      },
      {
        code: "supplier.return",
        label: "Post supplier returns",
        description: "Return goods to suppliers.",
      },
      {
        code: "payable.view",
        label: "View payables",
        description: "See supplier balances, statements, and aging.",
      },
    ],
  },
  {
    group: "Inventory",
    permissions: [
      {
        code: "inventory.adjust",
        label: "Adjust inventory",
        description: "Post stock adjustments and counts.",
      },
    ],
  },
  {
    group: "Finance",
    permissions: [
      { code: "profit.view", label: "View profit", description: "See profit and margin reports." },
    ],
  },
  {
    group: "Reports",
    permissions: [
      { code: "report.export", label: "Export reports", description: "Export reports and data." },
    ],
  },
  {
    group: "Users & security",
    permissions: [
      { code: "user.manage", label: "Manage users", description: "List and view user accounts." },
      { code: "user.create", label: "Create users", description: "Add new user accounts." },
      { code: "user.edit", label: "Edit users", description: "Edit user details and roles." },
      { code: "user.deactivate", label: "Deactivate users", description: "Deactivate user accounts." },
      {
        code: "user.reset_password",
        label: "Reset passwords",
        description: "Reset another user's password.",
      },
      { code: "role.manage", label: "Manage roles", description: "Edit role permissions." },
    ],
  },
  {
    group: "Settings & data",
    permissions: [
      {
        code: "settings.manage",
        label: "Change settings",
        description: "Change shop and application settings.",
      },
      {
        code: "settings.danger",
        label: "Dangerous settings",
        description: "Change inventory/accounting-affecting settings.",
      },
      { code: "backup.create", label: "Create backups", description: "Run manual backups." },
      { code: "backup.restore", label: "Restore backups", description: "Restore a backup." },
    ],
  },
  {
    group: "Audit",
    permissions: [
      { code: "audit.view", label: "View audit log", description: "Read the audit log." },
      { code: "audit.export", label: "Export audit log", description: "Export audit events." },
    ],
  },
];

export const ALL_PERMISSIONS: PermissionDef[] = PERMISSION_GROUPS.flatMap(
  (g) => g.permissions,
);

export function permissionLabel(code: string): string {
  return ALL_PERMISSIONS.find((p) => p.code === code)?.label ?? code;
}