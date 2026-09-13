const fs = require('fs');
const path = require('path');
const file = path.join('d:', 'Projects', 'Furniture Showroom system', 'src', 'components', 'sales', 'sales-page.tsx');
let content = fs.readFileSync(file, 'utf8');
const lines = content.split('\n');
console.log('1345: ' + lines[1344]);
console.log('1707: ' + lines[1706]);
console.log('1754: ' + lines[1753]);
