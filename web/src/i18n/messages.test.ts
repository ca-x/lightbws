import { describe, expect, it } from "vitest"

import { messages } from "./messages"

describe("translations", () => {
  it("keeps both locales complete and non-empty", () => {
    expect(Object.keys(messages["zh-CN"]).sort()).toEqual(Object.keys(messages.en).sort())
    expect(Object.values(messages.en).every((value) => value.trim().length > 0)).toBe(true)
    expect(Object.values(messages["zh-CN"]).every((value) => value.trim().length > 0)).toBe(true)
  })

  it("uses singular nouns in create actions", () => {
    expect(`${messages.en.new} ${messages.en.secret}`).toBe("New secret")
    expect(`${messages.en.new} ${messages.en.machineAccount}`).toBe("New machine account")
    expect(`${messages.en.new} ${messages.en.user}`).toBe("New user")
    expect(messages.en.newBackupTarget).toBe("New backup target")
  })

  it("describes Web-created projects as BWS-compatible", () => {
    expect(messages.en.newProjectHint).toContain("BWS")
    expect(messages["zh-CN"].newProjectHint).toContain("BWS")
    expect(messages.en.helpAutomationIntro).toContain("created in Web")
    expect(messages["zh-CN"].helpAutomationIntro).toContain("网页")
  })
})
