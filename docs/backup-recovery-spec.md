# Backup scopes and recovery specification

## Objective

Extend automatic backups and manual transfer from project-and-secret snapshots to selectable logical backups. The existing project-and-secret scope remains the default. Administrators can select identities, groups, machine accounts, access policies, audit data, and backup target configuration, or use a full-instance preset.

## Formats and encryption

- Existing `LBWSX01` passphrase exports and `LIGHTBWS-BACKUP-V1` automatic backups remain importable.
- New archives carry a versioned manifest with their selected scopes and encryption mode.
- Manual exports are passphrase encrypted by default.
- Automatic backups are encrypted with the instance master key by default. The master key is never uploaded or embedded.
- `LIGHTBWS_ALLOW_PLAINTEXT_BACKUPS=false` by default. Setting it to `true` unlocks, but does not select, plaintext mode.
- Plaintext mode is selected per backup target or manual export, requires explicit confirmation, and uses a visibly named `.plain.lightbws` file.
- Plaintext imports require neither a passphrase nor the source master key. Encrypted automatic imports require the source master key. Passphrase exports require their passphrase.

## Scopes

Projects and secrets are always selected. Optional scopes are:

- identities: users, groups, and group membership;
- machine accounts;
- access policies for projects, secrets, and machine accounts;
- audit settings and audit events;
- backup target configuration and credentials.

The full-instance preset selects every persistent scope. Sessions, machine sessions, backup job history, migration metadata, SQLite WAL state, and the master key are never included.

Groups require identities. Machine accounts require identities. Access policies require identities, groups, and machine accounts. The API rejects invalid combinations instead of silently expanding them.

## Restore semantics

- Normal imports merge by identifier in one transaction. Unique-key conflicts or invalid references reject and roll back the entire import.
- Full-instance imports replace all included persistent scopes in dependency order and complete in one transaction.
- Backup credentials are plaintext only inside the already protected archive payload. On import they are encrypted with the destination instance master key before being stored.
- Full-instance restore invalidates old sessions and requires the administrator to sign in again.
- Input size, schema, identifiers, field sizes, enum values, scope dependencies, and encryption credentials are validated before mutation.

## UI requirements

- Backup target and manual export forms expose scope presets and individual scopes.
- Encryption mode defaults to encrypted. Plaintext is unavailable unless the server capability is enabled.
- Plaintext selection shows a persistent danger banner and requires confirmation.
- Backup cards show encryption and scope status.
- The import form detects the archive type and requests a passphrase, source master key, or no credential as appropriate.
- Information icons next to backup content, encryption mode, and import credential explain the recovery consequences in Chinese and English.
- Tips work with hover, keyboard focus, and touch/click, have accessible names, and do not rely on color alone.

## Commands and structure

- Format: `cargo fmt --all -- --check`
- Rust checks: `cargo clippy --all-targets --all-features -- -D warnings`
- Rust tests: `cargo test --all-targets --all-features`
- Web checks: `npm --prefix web run typecheck && npm --prefix web run test:ci && npm --prefix web run build`
- Backend format and restore logic lives in `src/domain/transfer.rs`; target execution stays in `src/domain/backups.rs`; HTTP contracts stay under `src/api/`.
- UI contracts live in `web/src/app/types.ts` and `web/src/app/api.ts`; presentation and bilingual copy live in `web/src/app/App.tsx` and `web/src/i18n/messages.ts`.

## Boundaries

- Always: require administrator authorization, validate server-side, keep default encryption, use transactional imports, redact credentials and keys from logs and responses, and preserve old archive imports.
- Ask first: changing the default scope, enabling plaintext without an environment gate, or restoring sessions.
- Never: upload or embed `master.key`, log archive content or credentials, silently downgrade an existing target to plaintext, or partially commit a failed import.

## Success criteria

- Existing targets and manual exports keep their current encrypted project-and-secret behavior.
- Every valid scope combination round-trips through encrypted and permitted plaintext archives.
- A full plaintext archive reconstructs all persistent application data on a fresh instance while using the destination master key for stored credentials.
- Legacy automatic archives import with the source master key; legacy manual exports import with their passphrase.
- Forged plaintext target/export requests fail while the environment gate is disabled.
- Rust, Web, formatting, lint, and security checks pass.
