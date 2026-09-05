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
  it.each([
    "0;0%", "0%;0", "0%;0%;0", "0%;;0%", "0;[$¥-411]0",
    "[$¥-411]0;[$$-409]0", "[$¥-411]0;[$¥-804]0",
    "[$¥-411]$0", "[>=0]0%;0%", "[<1]0%;0", '0%"unterminated',
  ])("falls back for differing, conditional or malformed numeric sections: %s", pattern => {
    expect(importedNumberFormat(pattern)).toBe("number");
  });
  it.each([
    ["0;0;0;@%", "number"],
    ["0%;0%;0%;@", "percentage"],
    ['0%";literal";0%', "percentage"],
    ["0%\\;;0%", "percentage"],
    ["0%_;;0%", "percentage"],
    ["0%*;;0%", "percentage"],
    ["[Red;Blue]0%;0%", "percentage"],
    ["[$¥-411]0;[$JPY-409]0;¥0;@$", "currency-jpy"],
    ['0";%";0;0;@%', "number"],
  ])("respects section grammar without interpreting the text section: %s", (pattern, expected) => {
    expect(importedNumberFormat(pattern)).toBe(expected);
  });

  it("keeps escaped quotes and their enclosed operators inside the literal", () => {
    expect(importedNumberFormat(String.raw`0"a\"%\"b"`)).toBe("number");
    expect(importedNumberFormat(String.raw`0"a\";0%\"b";0`)).toBe("number");
    expect(importedNumberFormat(String.raw`0"a\"%\"b"%`)).toBe("percentage");
    expect(importedNumberFormat(String.raw`0"a\"`)).toBe("number");
    expect(importedNumberFormat('"¥"0')).toBe("currency-jpy");
    expect(importedNumberFormat('"$"0')).toBe("currency-usd");
  });

});
