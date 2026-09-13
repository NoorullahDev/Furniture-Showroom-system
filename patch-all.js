const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'components', 'sales', 'sales-page.tsx');
let content = fs.readFileSync(file, 'utf8');

// 1. Imports
content = content.replace(
  'import { MoreHorizontal } from "lucide-react";',
  'import { MoreHorizontal } from "lucide-react";\nimport { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";'
);
content = content.replace('saleCreate,', 'saleCreate,\n  saleUpdate,');

// 2. State
content = content.replace(
  'const session = profile?.sessionId ?? "";',
  'const session = profile?.sessionId ?? "";\n  const [editSaleId, setEditSaleId] = React.useState<number | null>(null);'
);

// 3. View Conditionals
content = content.replace('{activeTab === "sales" && (\n        <div className="mt-5 grid grid-cols-2 gap-3', '{activeTab === "sales" && !editSaleId && (\n        <div className="mt-5 grid grid-cols-2 gap-3');
content = content.replace('{activeTab === "due" && (\n        <div className="mt-5 grid grid-cols-2 gap-3', '{activeTab === "due" && !editSaleId && (\n        <div className="mt-5 grid grid-cols-2 gap-3');
content = content.replace('{activeTab === "sales" && (\n          <div>\n            {dashboardSalesFilter', '{activeTab === "sales" && !editSaleId && (\n          <div>\n            {dashboardSalesFilter');
content = content.replace('{activeTab === "customers" && (\n          <CustomersTable', '{activeTab === "customers" && !editSaleId && (\n          <CustomersTable');
content = content.replace('{activeTab === "due" && canReceive && <DueControlPanel session={session} onError={failed} />}', '{activeTab === "due" && !editSaleId && canReceive && <DueControlPanel session={session} onError={failed} />}');

// 4. PosPanel Mount & Props
content = content.replace('{activeTab === "pos" && (\n          <PosPanel', '{(activeTab === "pos" || editSaleId) && (\n          <PosPanel\n            editSaleId={editSaleId}\n            onBack={editSaleId ? () => setEditSaleId(null) : undefined}');
content = content.replace('onDone={done("Sale confirmed")}', 'onDone={() => {\n              toast({ variant: "success", title: editSaleId ? "Sale updated" : "Sale confirmed" });\n              setEditSaleId(null);\n              refresh();\n            }}');
content = content.replace('onEdit={(s) => setEditSaleId(s.id)}\n              onView={(s) => {', 'onView={(s) => {'); // remove if accidentally added
content = content.replace('canPrint={canPrint}\n              onView={(s) => {', 'canPrint={canPrint}\n              onEdit={(s) => setEditSaleId(s.id)}\n              onView={(s) => {');

// 5. PosPanel Signature
content = content.replace(
  'canPrint: boolean;\n  canSell: boolean;\n  onNewCustomer: () => void;\n  onDone: () => void;\n  onFailed: (e: Error) => void;\n}) {',
  'canPrint: boolean;\n  canSell: boolean;\n  editSaleId?: number | null;\n  onBack?: () => void;\n  onNewCustomer: () => void;\n  onDone: () => void;\n  onFailed: (e: Error) => void;\n}) {'
);
content = content.replace(
  'canSell,\n  onNewCustomer,\n  onDone,\n  onFailed,\n}: {',
  'canSell,\n  editSaleId,\n  onBack,\n  onNewCustomer,\n  onDone,\n  onFailed,\n}: {'
);

// 6. PosPanel useEffect
const useEffectStr = `
  React.useEffect(() => {
    if (!editSaleId || !session) return;
    let mounted = true;
    saleGet(session, editSaleId).then((sale) => {
      if (!mounted) return;
      setCustomerId(sale.customerId ?? null);
      setDiscountMinor(sale.discountMinor ?? 0);
      setDeliveryMinor(sale.deliveryChargeMinor ?? 0);
      
      const cartItems = sale.items.map((i, idx) => ({
        key: 'edit-' + idx,
        productId: i.productId ?? undefined,
        bundleId: i.bundleId ?? undefined,
        name: i.productName ?? 'Unknown item',
        article: i.articleNumber ?? '',
        unitPriceMinor: i.unitPriceMinor,
        quantity: i.quantity,
      }));
      setCart(cartItems);
      
      setPaidMinor(sale.paidMinor ?? 0);
      setAdvanceMinor(sale.advanceUsedMinor ?? 0);
    }).catch(console.error);
    return () => { mounted = false; };
  }, [editSaleId, session]);
`;
content = content.replace('const stockByProduct: Record<number, StockBalanceDto> = React.useMemo(() => {', useEffectStr + '\n  const stockByProduct: Record<number, StockBalanceDto> = React.useMemo(() => {');

// 7. PosPanel confirmMutation
const confirmOld = `const confirmMutation = useMutation({
    mutationFn: async () => {
      if (!locationId || cart.length === 0) throw new Error("no items");
      let saleId = createdIdRef.current;
      if (!saleId) {
        const draft = await saleCreate(session, {`;
        
const confirmNew = `const confirmMutation = useMutation({
    mutationFn: async () => {
      if (!locationId || cart.length === 0) throw new Error("no items");
      if (editSaleId) {
        return await saleUpdate(session, {
          saleId: editSaleId,
          customerId,
          discountMinor: discountMinor > 0 ? discountMinor : null,
          deliveryChargeMinor: deliveryMinor > 0 ? deliveryMinor : null,
          items: cart.map((l) => ({
            productId: l.productId ?? null,
            bundleId: l.bundleId ?? null,
            quantity: l.quantity,
          })),
          paidMinor: paidMinor > 0 ? paidMinor : null,
          cashAccountId: paidMinor > 0 ? accountId : null,
          paymentMethodId: paidMinor > 0 ? methodId : null,
          advanceUsedMinor: advanceMinor > 0 ? advanceMinor : null,
          notes: null,
        });
      }
      let saleId = createdIdRef.current;
      if (!saleId) {
        const draft = await saleCreate(session, {`;

content = content.replace(confirmOld, confirmNew);

// 8. SalesTable Signatures
content = content.replace(
  'canPrint: boolean;\n  onView: (s: SaleDto) => void;\n  onCancel: (s: SaleDto) => void;\n}) {',
  'canPrint: boolean;\n  onEdit: (s: SaleDto) => void;\n  onView: (s: SaleDto) => void;\n  onCancel: (s: SaleDto) => void;\n}) {'
);
content = content.replace(
  'canPrint,\n  onView,\n  onCancel,\n}: {',
  'canPrint,\n  onEdit,\n  onView,\n  onCancel,\n}: {'
);

// 9. SalesTable Date format
content = content.replace('<TableCell className="whitespace-nowrap">{s.saleDate}</TableCell>', '<TableCell className="whitespace-nowrap">{s.saleDate ? new Date(s.saleDate).toLocaleDateString(undefined, { dateStyle: "medium" }) : "—"}</TableCell>');

// 10. SalesTable Actions
const actionsOld = `<div className="flex items-center justify-end gap-2">
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
                  </div>`;

const actionsNew = `<div className="flex items-center justify-end gap-2">
                    <Button variant="outline" size="sm" onClick={() => onView(s)}>
                      View
                    </Button>
                    {(s.status === "confirmed" || s.status === "draft") && (
                      <Button variant="outline" size="sm" onClick={() => onEdit(s)}>
                        {s.status === "draft" ? "Resume" : "Edit"}
                      </Button>
                    )}
                    {canPrint && s.status === "confirmed" && (
                      <PrintInvoiceButton session={session} saleId={s.id} />
                    )}
                    {(s.status === "confirmed" || s.status === "draft") && (
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
                    )}
                  </div>`;
content = content.replace(actionsOld, actionsNew);

fs.writeFileSync(file, content, 'utf8');
console.log('sales-page.tsx patched flawlessly');
