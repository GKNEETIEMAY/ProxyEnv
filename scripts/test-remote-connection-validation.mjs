import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const baseline = JSON.parse(readFileSync(new URL("./fixtures/remote-connection-validation-baseline.json", import.meta.url), "utf8"));
const source = readFileSync(new URL("../src/features/remote-bridge/connection-validation.ts", import.meta.url), "utf8");
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
});
const validators = await import("data:text/javascript;base64," + Buffer.from(outputText).toString("base64"));

for (const [name, { valid, invalid }] of Object.entries(baseline)) {
  test("remote connection validation preserves the baseline for " + name, () => {
    for (const input of valid) {
      assert.equal(validators[name](input), true, JSON.stringify(input));
    }
    for (const input of invalid) {
      assert.equal(validators[name](input), false, JSON.stringify(input));
    }
  });
}
