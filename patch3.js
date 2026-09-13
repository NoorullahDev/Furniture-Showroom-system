const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'components', 'sales', 'sales-page.tsx');
let content = fs.readFileSync(file, 'utf8');

if (!content.includes('saleUpdate,')) {
  content = content.replace('saleCreate,', 'saleCreate,\n  saleUpdate,');
}

const useEffectHook = `
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
        name: i.productName ?? i.bundleName ?? 'Unknown item',
        article: i.productArticle ?? '',
        unitPriceMinor: i.unitPriceMinor,
        quantity: i.quantity,
      }));
      setCart(cartItems);
      
      const payments = (sale.payments ?? []).reduce((acc, p) => acc + p.amountMinor, 0);
      const advance = sale.advanceUsedMinor ?? 0;
      setPaidMinor(payments);
      setAdvanceMinor(advance);
      
      if (sale.payments && sale.payments.length > 0) {
        setMethodId(sale.payments[0].paymentMethodId);
        setAccountId(sale.payments[0].cashAccountId);
      }
    }).catch(console.error);
    return () => { mounted = false; };
  }, [editSaleId, session]);
`;

if (!content.includes('let mounted = true;')) {
  content = content.replace(
    'const stockByProduct: Record<number, StockBalanceDto> = React.useMemo(() => {',
    useEffectHook + '\n  const stockByProduct: Record<number, StockBalanceDto> = React.useMemo(() => {'
  );
}

const confirmMutationOld = `const confirmMutation = useMutation({
    mutationFn: async () => {
      if (!locationId || cart.length === 0) throw new Error("no items");
      let saleId = createdIdRef.current;
      if (!saleId) {
        const draft = await saleCreate(session, {
          locationId,
          customerId,
          kind: "sale",
          discountMinor: discountMinor > 0 ? discountMinor : null,
          deliveryChargeMinor: deliveryMinor > 0 ? deliveryMinor : null,
          items: cart.map((l) => ({
            productId: l.productId ?? null,
            bundleId: l.bundleId ?? null,
            quantity: l.quantity,
          })),
        });
        saleId = draft.id;
        createdIdRef.current = saleId;
      }
      return saleConfirm(session, {
        saleId,
        idempotencyKey: \`sale-confirm-\${saleId}-\${Date.now()}\`,
        paidMinor: paidMinor > 0 ? paidMinor : null,
        cashAccountId: paidMinor > 0 ? accountId : null,
        paymentMethodId: paidMinor > 0 ? methodId : null,
        advanceUsedMinor: advanceMinor > 0 ? advanceMinor : null,
      });
    },`;

const confirmMutationNew = `const confirmMutation = useMutation({
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
        const draft = await saleCreate(session, {
          locationId,
          customerId,
          kind: "sale",
          discountMinor: discountMinor > 0 ? discountMinor : null,
          deliveryChargeMinor: deliveryMinor > 0 ? deliveryMinor : null,
          items: cart.map((l) => ({
            productId: l.productId ?? null,
            bundleId: l.bundleId ?? null,
            quantity: l.quantity,
          })),
        });
        saleId = draft.id;
        createdIdRef.current = saleId;
      }
      return saleConfirm(session, {
        saleId,
        idempotencyKey: \`sale-confirm-\${saleId}-\${Date.now()}\`,
        paidMinor: paidMinor > 0 ? paidMinor : null,
        cashAccountId: paidMinor > 0 ? accountId : null,
        paymentMethodId: paidMinor > 0 ? methodId : null,
        advanceUsedMinor: advanceMinor > 0 ? advanceMinor : null,
      });
    },`;

if (!content.includes('if (editSaleId) {\n        return await saleUpdate(')) {
  content = content.replace(confirmMutationOld, confirmMutationNew);
}

fs.writeFileSync(file, content, 'utf8');
console.log('PosPanel logic patched');
