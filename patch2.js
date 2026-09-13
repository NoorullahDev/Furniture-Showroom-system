const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'components', 'sales', 'sales-page.tsx');
let content = fs.readFileSync(file, 'utf8');

// Update PosPanel props signature
content = content.replace(
  'canSell,\\n  onNewCustomer,\\n  onDone,\\n  onFailed,\\n}: {',
  'canSell,\\n  editSaleId,\\n  onBack,\\n  onNewCustomer,\\n  onDone,\\n  onFailed,\\n}: {'
);
content = content.replace(
  'canPrint: boolean;\\n  canSell: boolean;\\n  onNewCustomer: () => void;\\n  onDone: () => void;\\n  onFailed: (e: Error) => void;\\n}) {',
  'canPrint: boolean;\\n  canSell: boolean;\\n  editSaleId?: number | null;\\n  onBack?: () => void;\\n  onNewCustomer: () => void;\\n  onDone: () => void;\\n  onFailed: (e: Error) => void;\\n}) {'
);

fs.writeFileSync(file, content, 'utf8');
console.log('PosPanel signature patched');
