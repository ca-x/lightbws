# Admin workflow refinement

## Scope and design decisions

- Preserve the existing LightBWS brand, Astryx themes, English/Chinese support, access model, and backend APIs.
- The primary audience is an instance administrator organizing secrets and connecting applications; members retain only their permitted actions.
- Put a single, discoverable creation entry in workspace navigation. Keep contextual creation buttons on resource pages, backed by the same workflow.
- Offer task-oriented choices: save a secret, organize a project, connect an application, add a user, and create a group. A secret never requires a project for administrators.
- After creating a project, offer adding a secret or configuring access without navigating away. Preserve saved resources if a later step fails or is cancelled.
- Allow creating a project inside the secret form without losing its draft. Offer project-scoped secret navigation with a shareable hash route and a clear way back.
- After machine creation, keep the one-time token available while configuring access. Never store tokens in browser storage.
- Group navigation by daily work, people/app access, and administration; keep help/settings easily discoverable. Make the mobile drawer modal, keyboard accessible, and safe-area aware.
- Use shared spacing, typography, semantic color, and motion tokens. Use short opacity/transform feedback for occasional overlays and completion; do not animate keyboard navigation or data filtering. Honor reduced motion, transparency, and high contrast.

## Implementation sequence

1. Update compatible frontend/backend dependencies and lockfiles, preserving the documented Rust MSRV.
2. Implement shared creation flows, contextual project creation, project-scoped secrets, and navigation refinements.
3. Refine alignment, touch targets, responsive layouts, and intentional interaction feedback.
4. Add regression tests for chained creation, preserved drafts, scoped navigation, role boundaries, and drawer keyboard behavior.
5. Ask independent agents to review the full implementation and dependency changes **before running tests**; address findings.
6. Run frontend tests/typecheck/build, Rust formatting/clippy/tests/release build, and SDK demo checks. Exercise real browser workflows at phone/tablet/desktop sizes, dark/light mode, and reduced motion.
7. Update synchronized package versions to the next patch release, document changes, commit, push, and publish the release tag. Verify GitHub release and image workflows rather than equating a tag push with completed publication.

## Review focus

- No accidental grants, member-only administrative actions, or token persistence.
- No duplicate creation after a follow-up failure; no lost secret drafts when creating a project inline.
- Browser back/forward preserves project scope; unknown/forbidden routes fall back safely.
- Drawer focus trapping, Escape/overlay dismissal, focus restoration, resize cleanup, and background scroll locking.
- Long names, translated labels, 320–400px layouts, landscape, larger text, and independent table scrolling.

## Acceptance evidence

Record actual review findings, command results, browser screenshots, release tag, workflow conclusions, and published assets in the release notes/session summary. Do not claim a check passed without running it.
