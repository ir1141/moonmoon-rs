import { describe, expect, test } from "bun:test";
import { EmoteLookupPolicy } from "./emote-lookup-policy.js";

describe("EmoteLookupPolicy", () => {
  test("puts failed providers and names on cooldown", () => {
    let now = 1_000;
    const policy = new EmoteLookupPolicy({
      providerCooldownMs: 10_000,
      nameCooldownMs: 3_000,
      now: () => now,
    });

    policy.recordFailure("7TV", "HoMM");

    expect(policy.canLookupName("HoMM")).toBe(false);
    expect(policy.availableProviders(["7TV", "BTTV"])).toEqual(["BTTV"]);

    now += 3_001;
    expect(policy.canLookupName("HoMM")).toBe(true);
    expect(policy.availableProviders(["7TV", "BTTV"])).toEqual(["BTTV"]);

    now += 7_000;
    expect(policy.availableProviders(["7TV", "BTTV"])).toEqual(["7TV", "BTTV"]);
  });

  test("successful provider response clears provider cooldown", () => {
    let now = 5_000;
    const policy = new EmoteLookupPolicy({
      providerCooldownMs: 10_000,
      now: () => now,
    });

    policy.recordFailure("BTTV", "catJAM");
    expect(policy.availableProviders(["BTTV"])).toEqual([]);

    policy.recordProviderSuccess("BTTV");
    expect(policy.availableProviders(["BTTV"])).toEqual(["BTTV"]);
  });

  test("can cool down a name without changing provider state", () => {
    let now = 10_000;
    const policy = new EmoteLookupPolicy({
      nameCooldownMs: 2_000,
      now: () => now,
    });

    policy.recordNameFailure("RETRY");

    expect(policy.canLookupName("RETRY")).toBe(false);
    expect(policy.availableProviders(["7TV", "BTTV"])).toEqual(["7TV", "BTTV"]);

    now += 2_001;
    expect(policy.canLookupName("RETRY")).toBe(true);
  });
});
