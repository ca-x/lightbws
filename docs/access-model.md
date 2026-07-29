# LightBWS Access Model

## Objective

LightBWS implements the official Bitwarden Secrets Manager resource and authorization model for one
organization: projects, secrets, organization users, groups, machine accounts, and access tokens.
The existing Bitwarden SDK and `bws` wire contracts remain compatible, including access-policy
contracts absent from the upstream fake server.

## Authoritative Product Rules

1. Secrets Manager has no personal secret space.
2. Every secret belongs to exactly one project.
3. Project access can be granted to organization users, groups, and machine accounts.
4. Secret access can also be granted directly to organization users, groups, and machine accounts.
5. Every grant has the official permission combination `read` or `read + write`; `write` without
   `read` is invalid.
6. Project and direct-secret grants are additive. A direct grant allows access even when the actor
   has no project grant.
7. Users inherit grants from every group they belong to.
8. `read` permits listing and retrieving the resource. `read + write` additionally permits creating,
   editing, and deleting secrets in the granted project.
9. A machine with write access may create a project through the existing SDK endpoint; LightBWS
   grants that machine read/write access to the new project. The explicit upstream compatibility
   account retains full SDK behavior for local compatibility tests.
10. Regular machine accounts start with no grants. Revocation and grant changes take effect without
    restarting LightBWS.
11. LightBWS Web `admin` users manage organization users, groups, machines, projects, and policies.
    Web `user` accounts are organization members and see only resources granted directly or through
    their groups.
12. Access decisions and policy changes are recorded in audit metadata without secret values.
13. Database correctness takes priority over compatibility with databases created by `v0.1.0`.

## Effective Access

For a Web user, effective access to a project is the union of direct user grants and inherited group
grants. Effective access to a secret is the union of:

- its project's effective user/group grants; and
- direct secret grants to the user or any group containing the user.

For a machine account, effective secret access is the union of its project grant and direct secret
grant. Organization Web admins have full management access. The upstream compatibility account has
full SDK access but remains clearly identified as a compatibility/testing account.

All list, get, batch, project-secret, and sync operations filter before serialization. All mutations
authorize before modifying the database. Failed authorization returns `403` for a known authenticated
actor without disclosing secret content.

## Official SDK Contracts

Existing project, secret, sync, token, health, help, and echo routes remain wire-compatible.
Secret create/update accepts the generated SDK field:

```json
{
  "accessPoliciesRequests": {
    "userAccessPolicyRequests": [{ "granteeId": "uuid", "read": true, "write": false }],
    "groupAccessPolicyRequests": [{ "granteeId": "uuid", "read": true, "write": true }],
    "serviceAccountAccessPolicyRequests": [{ "granteeId": "uuid", "read": true, "write": true }]
  }
}
```

The generated Bitwarden access-policy routes implemented from the local SDK source are:

- `GET /api/organizations/{id}/access-policies/people/potential-grantees`
- `GET /api/organizations/{id}/access-policies/projects/potential-grantees`
- `GET /api/organizations/{id}/access-policies/service-accounts/potential-grantees`
- `GET|PUT /api/projects/{id}/access-policies/people`
- `GET|PUT /api/projects/{id}/access-policies/service-accounts`
- `GET /api/secrets/{id}/access-policies`
- `GET|PUT /api/service-accounts/{id}/granted-policies`
- `GET|PUT /api/service-accounts/{id}/access-policies/people`

Generated response field names and object wrappers are preserved. Empty request arrays replace the
corresponding policy set. Replacement is transactional and rejects unknown grantees or invalid
permission pairs before changing any grant.

## LightBWS Web Contracts

- Project responses include effective `read` and `write` permissions.
- Secret responses include effective `read` and `write` permissions.
- `GET|PUT /api/v1/projects/{id}/access` manages user, group, and machine grants.
- `GET|PUT /api/v1/secrets/{id}/access` manages direct user, group, and machine grants.
- `GET|PUT /api/v1/machines/{id}/access` atomically manages user/group visibility and project grants.
- `GET|POST /api/v1/admin/groups` lists and creates groups.
- `PUT|DELETE /api/v1/admin/groups/{id}` edits or removes groups.
- `GET|PUT /api/v1/admin/groups/{id}/members` lists or replaces group membership.
- Existing user and machine management routes remain compatible.
- `GET|DELETE /api/v1/audit` lists or manually clears admin-visible events without secret values.
- `GET|PUT /api/v1/audit/settings` controls collection, hourly retention cleanup, and the 1-3650 day
  retention period.

All Web errors use the existing structured `AppError` response and distinguish authentication,
authorization, missing resources, invalid requests, and conflicts.

## Data Model

- `users`: local login identities and the existing `admin`/`user` management role.
- `groups`: organization groups.
- `group_members`: unique user/group membership pairs.
- `projects`: organization-owned logical secret containers.
- `secrets.project_id`: required project ownership.
- `project_user_grants`, `project_group_grants`, `project_machine_grants`: read/write policies.
- `secret_user_grants`, `secret_group_grants`, `secret_machine_grants`: direct read/write policies.
- `machine_accounts`: access-token identities plus an explicit compatibility-account marker.
- `audit_events`: Web and machine access/policy events with actor and outcome metadata.

Foreign keys cascade grants and memberships when their parent is removed. Unique composite keys
prevent duplicate grants. Database checks enforce `write -> read`.

## Threat Model and Abuse Cases

Assets are secret ciphertext/plaintext, machine credentials, sessions, grants, group membership, and
audit metadata. Trust boundaries are Web sessions, CSRF headers, SDK bearer tokens, JSON/form bodies,
imports, and database rows.

Required abuse-case coverage:

- A user cannot enumerate a project or secret without a direct or inherited grant.
- Group membership grants and revocations take effect immediately.
- A read-only user or machine cannot create, edit, or delete secrets.
- A machine with no grant receives no projects/secrets from list, batch, or sync endpoints.
- Direct secret access works without a project grant and does not expose sibling secrets.
- Batch and sync endpoints cannot bypass per-resource authorization.
- Policy replacement rejects unknown grantees and never partially applies.
- Audit events never contain secret key, value, note, ciphertext, access token, or password.

## Commands

- Format: `env -u RUSTUP_TOOLCHAIN cargo fmt --all --check`
- Rust lint: `env -u RUSTUP_TOOLCHAIN cargo clippy --all-targets --all-features -- -D warnings`
- Rust tests: `env -u RUSTUP_TOOLCHAIN cargo test --all-targets --all-features`
- Web install: `npm --prefix web ci --ignore-scripts`
- Web typecheck: `npm --prefix web run typecheck`
- Web tests: `npm --prefix web run test:ci`
- Web build: `npm --prefix web run build`
- Release build: `env -u RUSTUP_TOOLCHAIN cargo build --locked --release`
- SDK demo check: `env -u RUSTUP_TOOLCHAIN cargo check --manifest-path demo/sdk-demo/Cargo.toml`

## Success Criteria

- Every protected Web and SDK route uses centralized operation-specific authorization.
- Project, group inheritance, direct secret, and machine grants behave as defined above.
- Every secret has a valid project.
- Official generated access-policy request and response JSON is accepted and emitted.
- The Web UI manages users, groups, machines, project policies, and secret policies in Chinese and
  English using built-in Astryx themes.
- Existing embedded Web, binary, Docker, backup, transfer, SDK demo, and release checks still pass.
