const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'components', 'sales', 'sales-page.tsx');
let content = fs.readFileSync(file, 'utf8');

content = content.replace(/\{s\.saleDate \? new Date\(s\.saleDate\)\.toLocaleDateString\(undefined, \{ dateStyle: \"medium\" \}\) : \"\"\}/g, '{s.saleDate ? new Date(s.saleDate).toLocaleDateString(undefined, { dateStyle: "medium" }) : "—"}');
content = content.replace(/\{s\.customerName \?\? "\?""\}/g, '{s.customerName ?? "—"}');

fs.writeFileSync(file, content, 'utf8');
console.log('Fixed syntax errors');
