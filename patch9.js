const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'components', 'sales', 'sales-page.tsx');
let content = fs.readFileSync(file, 'utf8');

const searchStr = '      let saleId = createdIdRef.current;\n      if (!saleId) {\n        const draft = await saleCreate(session, {';

const replaceStr = '      if (editSaleId) {\n        return await saleUpdate(session, {\n          saleId: editSaleId,\n          customerId,\n          discountMinor: discountMinor > 0 ? discountMinor : null,\n          deliveryChargeMinor: deliveryMinor > 0 ? deliveryMinor : null,\n          items: cart.map((l) => ({\n            productId: l.productId ?? null,\n            bundleId: l.bundleId ?? null,\n            quantity: l.quantity,\n          })),\n          paidMinor: paidMinor > 0 ? paidMinor : null,\n          cashAccountId: paidMinor > 0 ? accountId : null,\n          paymentMethodId: paidMinor > 0 ? methodId : null,\n          advanceUsedMinor: advanceMinor > 0 ? advanceMinor : null,\n          notes: null,\n        });\n      }\n\n      let saleId = createdIdRef.current;\n      if (!saleId) {\n        const draft = await saleCreate(session, {';

if (content.includes(searchStr)) {
  content = content.replace(searchStr, replaceStr);
  fs.writeFileSync(file, content, 'utf8');
  console.log('Patched confirmMutation');
} else {
  console.log('Could not find search string');
}
