const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'components', 'sales', 'sales-page.tsx');
let content = fs.readFileSync(file, 'utf8');

// 1. Add DropdownMenu imports
content = content.replace(
  'import { MoreHorizontal } from "lucide-react";',
  'import { MoreHorizontal } from "lucide-react";\nimport { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";'
);

// 2. Add editSaleId state to SalesPage
content = content.replace(
  'const session = profile?.sessionId ?? "";',
  'const session = profile?.sessionId ?? "";\n  const [editSaleId, setEditSaleId] = React.useState<number | null>(null);'
);

// 3. Hide other tabs when editSaleId is set
content = content.replace(
  '{activeTab === "sales" && (\\n        <div className="mt-5 grid grid-cols-2 gap-3',
  '{activeTab === "sales" && !editSaleId && (\\n        <div className="mt-5 grid grid-cols-2 gap-3'
);
content = content.replace(
  '{activeTab === "due" && (\\n        <div className="mt-5 grid grid-cols-2 gap-3',
  '{activeTab === "due" && !editSaleId && (\\n        <div className="mt-5 grid grid-cols-2 gap-3'
);
content = content.replace(
  '{activeTab === "sales" && (\\n          <div>\\n            {dashboardSalesFilter',
  '{activeTab === "sales" && !editSaleId && (\\n          <div>\\n            {dashboardSalesFilter'
);
content = content.replace(
  '{activeTab === "customers" && (\\n          <CustomersTable',
  '{activeTab === "customers" && !editSaleId && (\\n          <CustomersTable'
);
content = content.replace(
  '{activeTab === "due" && canReceive && <DueControlPanel',
  '{activeTab === "due" && !editSaleId && canReceive && <DueControlPanel'
);

// 4. Show PosPanel when editSaleId is set
content = content.replace(
  '{activeTab === "pos" && (\\n          <PosPanel',
  '{(activeTab === "pos" || editSaleId) && (\\n          <PosPanel\\n            editSaleId={editSaleId}\\n            onBack={editSaleId ? () => setEditSaleId(null) : undefined}'
);

// 5. Update PosPanel onDone to handle edit success
content = content.replace(
  'onDone={done("Sale confirmed")}',
  'onDone={() => {\\n              toast({ variant: "success", title: editSaleId ? "Sale updated" : "Sale confirmed" });\\n              setEditSaleId(null);\\n              refresh();\\n            }}'
);

// 6. Pass onEdit to SalesTable
content = content.replace(
  'canPrint={canPrint}\\n              onView={(s) => {',
  'canPrint={canPrint}\\n              onEdit={(s) => setEditSaleId(s.id)}\\n              onView={(s) => {'
);

// 7. Update SalesTable signature
content = content.replace(
  'canPrint,\\n  onView,\\n  onCancel,\\n}: {',
  'canPrint,\\n  onEdit,\\n  onView,\\n  onCancel,\\n}: {'
);
content = content.replace(
  'canPrint: boolean;\\n  onView: (s: SaleDto) => void;\\n  onCancel: (s: SaleDto) => void;\\n}) {',
  'canPrint: boolean;\\n  onEdit: (s: SaleDto) => void;\\n  onView: (s: SaleDto) => void;\\n  onCancel: (s: SaleDto) => void;\\n}) {'
);

// 8. Update Date rendering in SalesTable
content = content.replace(
  '<TableCell className="whitespace-nowrap">{s.saleDate}</TableCell>',
  '<TableCell className="whitespace-nowrap">{s.saleDate ? new Date(s.saleDate).toLocaleDateString(undefined, { dateStyle: "medium" }) : "—"}</TableCell>'
);

// 9. Update SalesTable Actions
const oldActions = \<div className="flex items-center justify-end gap-2">
                    <Button variant="outline" size="sm" onClick={() => onView(s)}>
                      View
                    </Button>
                    {canPrint && s.status === "confirmed" && (
                      <PrintInvoiceButton session={session} saleId={s.id} />
                    )}
                    {canCancel && s.status === "confirmed" && (
                      <Button variant="outline" size="sm" className="text-rose-600" onClick={() => onCancel(s)}>
                        Cancel
                      </Button>
                    )}
                  </div>\;

const newActions = \<div className="flex items-center justify-end gap-2">
                    <Button variant="outline" size="sm" onClick={() => onView(s)}>
                      View
                    </Button>
                    {(s.status === "confirmed" || s.status === "draft") && (
                      <Button variant="outline" size="sm" onClick={() => onEdit(s)}>
                        {s.status === "draft" ? "Resume/Edit" : "Edit"}
                      </Button>
                    )}
                    {canPrint && s.status === "confirmed" && (
                      <PrintInvoiceButton session={session} saleId={s.id} />
                    )}
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="sm" className="h-8 w-8 p-0">
                          <MoreHorizontal className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        {canCancel && s.status === "confirmed" && (
                          <DropdownMenuItem className="text-rose-600 focus:text-rose-600 focus:bg-rose-50 cursor-pointer" onClick={() => onCancel(s)}>
                            Cancel Sale
                          </DropdownMenuItem>
                        )}
                        {s.status === "draft" && (
                          <DropdownMenuItem className="text-rose-600 focus:text-rose-600 focus:bg-rose-50 cursor-pointer" onClick={() => onCancel(s)}>
                            Delete Draft
                          </DropdownMenuItem>
                        )}
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>\;
                  
content = content.replace(oldActions, newActions);

fs.writeFileSync(file, content, 'utf8');
console.log('sales-page.tsx patched successfully');
