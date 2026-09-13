const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'components', 'sales', 'sales-page.tsx');
let content = fs.readFileSync(file);
// Convert from buffer, remove null bytes, remove BOM if exists
let str = content.toString('utf8').replace(/\0/g, '');
if (str.charCodeAt(0) === 0xFEFF) {
  str = str.slice(1);
}
fs.writeFileSync(file, str, 'utf8');
console.log('Cleaned binary chars');
