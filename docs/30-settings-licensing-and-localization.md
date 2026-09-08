# Settings, Licensing, and Localization

## Shop settings

- Business name, logo, address, phone, email, tax/registration numbers.
- Currency (default PKR), number/date format, timezone.
- Invoice prefix/sequence and terms.
- Default location, payment account, tax profile, and payment method.
- Inventory issue policy, negative-stock rule, minimum margin, discount/approval limits.
- Invoice/report paper size and printer defaults.
- Backup schedule, destination, and retention.

Settings changes are typed, validated, authorized, and audited. Settings affecting accounting or inventory should be versioned by effective date where historical interpretation matters.

## Licensing

Licensing is optional for an internal-only product but recommended when EagleNest Creations distributes it commercially.

Suggested states: trial, active, grace, expired, suspended, and invalid. Store a signed license payload containing license ID, customer/shop, product edition, issue/expiry, permitted features, and optional device binding. Verify signatures locally with an embedded public key; never embed the private signing key.

## Offline activation

Support request/response activation files or codes for shops without internet. Device binding should tolerate normal updates but have a documented reset process for hardware replacement. Store license state securely and audit activation changes.

## Expiry behavior

Never destroy or lock away the customer's data. On expiry, provide a clear grace period and then use a documented restricted mode such as read-only access plus backup/export, while blocking new commercial transactions. Emergency extension must be signed and time-limited.

## Branding

Maintain application product name and `Powered by EagleNest Creations` separately from customer invoice branding. Shop logo/contact appears on commercial documents. Prevent unlicensed white-label changes if editions differ.

## Localization

- Keep all UI strings outside components in translation dictionaries.
- Version 1 may ship English first, with Urdu-ready architecture.
- Support left-to-right and future right-to-left layout testing.
- Embed Unicode fonts in PDF output.
- Store user text in UTF-8 and test Urdu customer/product names.
- Format currency, dates, and numbers through locale utilities, while storing canonical values.

## First-run setup

1. Language.
2. Shop identity/logo.
3. Owner account.
4. Currency/date/timezone.
5. Location and inventory policy.
6. Invoice and printer defaults.
7. Backup destination.
8. License activation where applicable.

## Advanced settings safety

Separate everyday preferences from dangerous administrative settings. Show consequences and require owner confirmation for inventory policy, database restore, sequence changes, license operations, and financial reset. Do not provide a production `delete all data` action.

