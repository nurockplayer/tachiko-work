import { describe, expect, it } from "vitest";
import { importedNumberFormat } from "../src/interop-number-format.ts";

describe("bounded imported number presentation", () => {
  it.each(["[$¥-411]#,##0", "[$￥-411]#,##0", "[$JPY-409]0", '"¥"#,##0', "¥0", "\\¥0"])("recognizes JPY without treating locale punctuation as USD: %s", pattern => {
    expect(importedNumberFormat(pattern)).toBe("currency-jpy");
  });
  it.each(["[$$-409]#,##0.00", "[$USD-411]0.00", "$0.00", '"$"0.00'])("recognizes explicit supported USD: %s", pattern => {
    expect(importedNumberFormat(pattern)).toBe("currency-usd");
  });
  it.each(["[$¥-804]#,##0", "[$$-C09]0.00", "[$€-407]0.00", "[$-411]0.00", "0.00", '0"%"', "0\\%", '0"[$¥-411]"', "0_¥", "0*¥", "[$¥-411]0;[$$-409]0"])("does not invent a supported currency or percentage: %s", pattern => {
    expect(importedNumberFormat(pattern)).toBe("number");
  });
  it("recognizes the actual percentage operator and tolerates absent patterns", () => {
    expect(importedNumberFormat("[Red]0.00%")).toBe("percentage");
    expect(importedNumberFormat("[$-411]0.00%")).toBe("percentage");
    expect(importedNumberFormat("[$USD-409]0%")).toBe("number");
    expect(importedNumberFormat(null)).toBe("number");
    expect(importedNumberFormat(undefined)).toBe("number");
  });
});
