# Backup Scopes and Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add selectable logical backup scopes, full-instance recovery, legacy automatic import, and explicitly gated plaintext archives.

**Architecture:** A versioned manifest owns scope and encryption metadata. Transfer code serializes validated logical records and restores them transactionally; backup targets persist scope and encryption choices. HTTP and React surfaces expose the same contracts, with server capability gating plaintext mode.

**Tech Stack:** Rust 1.94, Axum, SeaORM 2, SQLite, AES-256-GCM, React 19, TypeScript 7, Vitest, Astryx Tooltip.

## Global Constraints

- Existing encrypted project-and-secret behavior remains the default.
- `LIGHTBWS_ALLOW_PLAINTEXT_BACKUPS` defaults to `false` and is enforced by the API.
- The master key, sessions, machine sessions, and backup jobs are never archived.
- Imports are all-or-nothing database transactions.
- No new dependencies.

---

### Task 1: Versioned archive contract

**Files:** `src/domain/transfer.rs`, `tests/backup_transfer.rs`

**Interfaces:** Produces `BackupScopes`, `ArchiveEncryption`, archive inspection, automatic decrypt, and versioned dump APIs.

- [x] Add failing tests proving default scopes, dependency validation, legacy automatic decryption, plaintext gating, and archive inspection.
- [x] Run `cargo test --test backup_transfer` and confirm the new tests fail on missing interfaces.
- [x] Implement the minimal versioned envelope and legacy-compatible decoders.
- [x] Run `cargo test --test backup_transfer` and confirm the slice passes.

### Task 2: Persistent logical data and transactional restore

**Files:** `src/domain/transfer.rs`, `src/domain/backups.rs`, `src/db/entities/*.rs`, `tests/backup_transfer.rs`

**Interfaces:** Consumes `BackupScopes`; produces scoped dump and merge/replace import functions that accept source and destination key context.

- [x] Add a failing integration test that creates every persistent data category, exports full scope, imports into a fresh database, and asserts the public repository/API-visible state.
- [x] Add a failing rollback test with one invalid record and assert no imported record remains.
- [x] Implement serialization, credential rewrapping, dependency ordering, and guarded audit replacement.
- [x] Run `cargo test --test backup_transfer --test access_control --test audit_retention`.

### Task 3: Target persistence and server capability

**Files:** `src/config.rs`, `src/lib.rs`, `src/db/migration.rs`, `src/db/entities/backup_target.rs`, `src/domain/backups.rs`, `src/api/backups.rs`, `tests/backup_transfer.rs`

**Interfaces:** Produces target fields `scopes` and `encryption`, plus `allow_plaintext_backups` capability.

- [x] Add failing tests for migration defaults and rejection of plaintext create/update while the capability is false.
- [x] Add a second migration and entity fields with encrypted/default scope values.
- [x] Thread the environment capability into `AppState` and enforce it in repository and execution paths.
- [x] Verify target defaults and automatic output filenames with `cargo test --test backup_transfer --test database_design`.

### Task 4: Transfer HTTP contract

**Files:** `src/api/transfer.rs`, `src/domain/transfer.rs`, `tests/backup_transfer.rs`

**Interfaces:** Produces archive inspection, scoped export, and credential-aware import endpoints.

- [x] Add failing router-level tests for passphrase, master-key, and plaintext imports, including admin authorization and size rejection.
- [x] Implement typed request/response models without logging secret fields.
- [x] Preserve legacy request compatibility where possible and return explicit validation codes.
- [x] Run the focused transfer API tests.

### Task 5: Web controls and accessible tips

**Files:** `web/src/app/types.ts`, `web/src/app/api.ts`, `web/src/app/App.tsx`, `web/src/i18n/messages.ts`, `web/src/styles/app.css`, `web/src/app/App.test.tsx`

**Interfaces:** Consumes backend capabilities and archive inspection; produces scope, encryption, confirmation, and credential UI.

- [x] Add failing Vitest cases for default scope, plaintext capability hiding, danger confirmation, archive detection, and accessible information tips.
- [x] Add typed API contracts and use Astryx Tooltip with an information IconButton.
- [x] Add bilingual copy, persistent warnings, badges, and responsive styles.
- [x] Run `npm --prefix web run typecheck && npm --prefix web run test:ci && npm --prefix web run build`.

### Task 6: Documentation and release verification

**Files:** `.env.example`, `README.md`, `README.zh-CN.md`, `docs/backup-recovery-spec.md`

**Interfaces:** Documents exact environment, recovery, scope, and plaintext risk contracts.

- [x] Update both READMEs and `.env.example` with defaults and recovery procedures.
- [x] Run bilingual punctuation gates and `git diff --check`.
- [x] Run Rust format, Clippy, all tests, release build, Web checks, and npm audit.
- [x] Review the final diff for secrets, accidental plaintext defaults, generated artifacts, and legacy compatibility.
