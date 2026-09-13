const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'lib', 'tauri', 'api.ts');
let content = fs.readFileSync(file, 'utf8');

const exportLine = "export type SaleUpdateInput = { saleId: number; customerId: number | null; discountMinor: number | null; deliveryChargeMinor: number | null; paidMinor: number | null; cashAccountId: number | null; paymentMethodId: number | null; advanceUsedMinor: number | null; notes: string | null; items: Array<{ productId: number | null; bundleId: number | null; quantity: number }> };";

const invokeLine = "export function saleUpdate(session: string, input: SaleUpdateInput) { return invoke<SaleDto>('sale_update', { session, input }); }";

if (!content.includes('SaleUpdateInput')) {
  fs.appendFileSync(file, '\n' + exportLine + '\n' + invokeLine + '\n');
}
