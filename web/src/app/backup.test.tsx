import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it } from "vitest"

import { Providers } from "./Providers"
import { LabelWithTip, canUsePlaintext, defaultBackupScopes, detectArchiveKind, normalizeScopes, plaintextSelectionReady, validateBackupForm, type BackupForm } from "./App"

const translateKey = (key: string) => key

function validS3Form(): BackupForm {
  return {
    kind: "S3",
    displayName: "Daily archive",
    endpoint: "https://s3.example.com",
    region: "us-east-1",
    bucket: "lightbws-backups",
    prefix: "daily/production",
    pathStyle: true,
    accessKeyId: "access-key",
    secretAccessKey: "secret-key",
    sessionToken: "",
    username: "",
    password: "",
    enabled: true,
    scheduleEnabled: true,
    intervalHours: "24",
    scopes: defaultBackupScopes(),
    encryption: "masterKey",
    confirmPlaintext: false,
  }
}

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

  it("mirrors server constraints before saving a backup target", () => {
    expect(validateBackupForm(validS3Form(), false, translateKey)).toEqual({})
    expect(validateBackupForm({ ...validS3Form(), prefix: " / " }, false, translateKey)).toEqual({})

    const invalid = { ...validS3Form(), endpoint: "http://s3.example.com/path", bucket: "x", intervalHours: "0" }
    expect(validateBackupForm(invalid, false, translateKey)).toMatchObject({
      endpoint: "backupEndpointRequirement",
      bucket: "backupBucketRequirement",
      intervalHours: "backupIntervalRequirement",
    })
    expect(validateBackupForm({ ...validS3Form(), prefix: "é".repeat(257) }, false, translateKey).prefix).toBe("backupPrefixRequirement")
  })

  it("requires a complete credential pair only when editing credentials", () => {
    const unchanged = { ...validS3Form(), accessKeyId: "", secretAccessKey: "" }
    expect(validateBackupForm(unchanged, true, translateKey)).toEqual({})

    const partial = { ...unchanged, accessKeyId: "replacement" }
    expect(validateBackupForm(partial, true, translateKey).secretAccessKey).toBe("backupCredentialPairRequirement")
  })

  it("allows WebDAV paths while keeping HTTPS mandatory", () => {
    const form: BackupForm = { ...validS3Form(), kind: "WEBDAV", endpoint: "https://dav.example.com/backups", username: "user", password: "secret" }
    expect(validateBackupForm(form, false, translateKey)).toEqual({})
  })
})
