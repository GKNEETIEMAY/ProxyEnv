import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const buildLog = process.argv[2];
assert.ok(buildLog, "Provide the Cargo JSON log from the isolated fixture build");
const artifacts = readFileSync(buildLog, "utf8").trim().split(/\r?\n/).map(line => JSON.parse(line));
const executable = artifacts.find(
  message => message.reason === "compiler-artifact" && message.target.name === "proxyenv_lib"
    && message.profile.test && message.executable,
)?.executable;
assert.ok(executable, "The isolated build must produce the proxyenv_lib test executable");

const probes = [
  "shutdown_under_store_contention",
  "repeated_shutdown_with_uncontended_store",
  "stale_profile_completion",
  "bridge_operation_admission_after_shutdown",
];
for (const probe of probes) {
  const name = `features::remote_bridge::tests::r46b_lifecycle_probes::${probe}`;
  for (let attempt = 1; attempt <= 3; attempt++) {
    const run = spawnSync(executable, ["--exact", name, "--nocapture"], {
      encoding: "utf8",
      timeout: 12_000,
      windowsHide: true,
    });
    process.stdout.write(`\n${probe} [${attempt}/3]\n${run.stdout ?? ""}${run.stderr ?? ""}`);
    if (run.error) throw run.error;
    assert.equal(run.status, 0, name);
    assert.match(run.stdout, /test result: ok\. 1 passed; 0 failed;/, name);
  }
}
