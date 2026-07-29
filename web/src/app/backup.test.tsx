import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it } from "vitest"

import { Providers } from "./Providers"
import { LabelWithTip, canUsePlaintext, defaultBackupScopes, detectArchiveKind, normalizeScopes, plaintextSelectionReady } from "./App"

describe("backup controls", () => {
  it("defaults to projects and secrets only", () => {
    expect(defaultBackupScopes()).toEqual({
      identities: false,
      machineAccounts: false,
      accessPolicies: false,
      audit: false,
      backupTargets: false,
    })
  })

  it("adds and removes scope dependencies", () => {
    const policy = normalizeScopes(defaultBackupScopes(), "accessPolicies", true)
    expect(policy).toMatchObject({ identities: true, machineAccounts: true, accessPolicies: true })
    expect(normalizeScopes(policy, "identities", false)).toMatchObject({
      identities: false,
      machineAccounts: false,
      accessPolicies: false,
    })
  })

  it("hides plaintext unless enabled and requires danger confirmation", () => {
    expect(canUsePlaintext({ plaintextAllowed: false })).toBe(false)
    expect(canUsePlaintext({ plaintextAllowed: true })).toBe(true)
    expect(plaintextSelectionReady(true, false)).toBe(false)
    expect(plaintextSelectionReady(true, true)).toBe(true)
    expect(plaintextSelectionReady(false, false)).toBe(true)
  })

  it.each([
    ["LBWSX01payload", "passphrase"],
    ["LIGHTBWS-BACKUP-V1\npayload", "masterKey"],
    ["LIGHTBWS-PLAIN-V2\npayload", "plaintext"],
    ["not-a-backup", "unknown"],
  ])("detects archive header %s", async (header, expected) => {
    expect(await detectArchiveKind(new File([header], "backup.lightbws"))).toBe(expected)
  })

  it("opens an accessible information tip on click", async () => {
    render(<Providers><LabelWithTip label="Archive protection" tip="Keep the key separately." /></Providers>)
    const trigger = screen.getByRole("button", { name: /more information: archive protection/i })
    expect(trigger).toHaveAttribute("aria-expanded", "false")
    await userEvent.click(trigger)
    expect(trigger).toHaveAttribute("aria-expanded", "true")
  })
})
