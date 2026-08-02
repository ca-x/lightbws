import { describe, expect, it } from "vitest"

import { messages } from "./messages"

describe("translations", () => {
  it("keeps both locales complete and non-empty", () => {
    expect(Object.keys(messages["zh-CN"]).sort()).toEqual(Object.keys(messages.en).sort())
    expect(Object.values(messages.en).every((value) => value.trim().length > 0)).toBe(true)
    expect(Object.values(messages["zh-CN"]).every((value) => value.trim().length > 0)).toBe(true)
  })

  it("uses locale-native create and edit actions", () => {
    expect(messages.en.newProject).toBe("New project")
    expect(messages.en.newSecret).toBe("New secret")
    expect(messages.en.newMachineAccount).toBe("New machine account")
    expect(messages.en.newUser).toBe("New user")
    expect(messages.en.newBackupTarget).toBe("New backup target")
    expect(messages["zh-CN"].newSecret).toBe("新建密钥")
    expect(messages["zh-CN"].editProject).toBe("编辑项目")
  })

  it("does not restate required fields as one-character minimums", () => {
    const keys = ["nameRequirement", "projectNameRequirement", "secretKeyRequirement"] as const
    const redundantMinimum = /(?:\b(?:use|enter)\s+|请输入\s*)1\s*(?:-|–|—|to|至)\s*[\d,]+\s*(?:characters|个字符)/iu
    for (const locale of ["en", "zh-CN"] as const) {
      for (const key of keys) expect(messages[locale][key]).not.toMatch(/\b1\s*(?:-|\u2013|to|至)\s*\d/u)
      for (const value of Object.values(messages[locale])) expect(value).not.toMatch(redundantMinimum)
    }
    expect(messages.en.nameRequirement).toBe("Up to 128 characters; no line breaks or control characters.")
    expect(messages["zh-CN"].secretKeyRequirement).toBe("最多 500 个字符；不支持换行和控制字符。")
  })

  it("describes every backup constraint needed to recover from an error", () => {
    expect(messages.en.backupBucketRequirement).toContain("consecutive periods")
    expect(messages.en.backupPrefixRequirement).toContain("backslashes")
    expect(messages.en.backupCredentialPairRequirement).toContain("every credential field")
    expect(messages["zh-CN"].backupBucketRequirement).toContain("连续的点")
    expect(messages["zh-CN"].backupPrefixRequirement).toContain("反斜杠")
    expect(messages["zh-CN"].keepCredentialsHint).toContain("所有凭据字段")
  })

  it("describes Web-created projects as BWS-compatible", () => {
    expect(messages.en.newProjectHint).toContain("BWS")
    expect(messages["zh-CN"].newProjectHint).toContain("BWS")
    expect(messages.en.helpAutomationIntro).toContain("created in Web")
    expect(messages["zh-CN"].helpAutomationIntro).toContain("网页")
  })
})
