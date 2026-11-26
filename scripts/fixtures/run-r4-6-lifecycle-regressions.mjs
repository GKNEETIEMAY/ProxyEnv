import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const [buildLog, ...requested] = process.argv.slice(2);
assert.ok(buildLog, "Provide the Cargo JSON log from the isolated fixture build");
const artifacts = readFileSync(buildLog, "utf8").trim().split(/\r?\n/).map(line => JSON.parse(line));
const executable = artifacts.find(
  message => message.reason === "compiler-artifact" && message.target.name === "proxyenv_lib"
    && message.profile.test && message.executable,
)?.executable;
assert.ok(executable, "The isolated build must produce the proxyenv_lib test executable");

const cases = [
  "shutdown_retires_transport_once_and_publishes_terminal_state",
  "shutdown_returns_while_store_is_held_and_reaps_after_release",
  "shutdown_drops_resources_outside_store_after_publishing",
  "shutdown_refuses_new_operations_and_releases_existing_operation",
  "monitor_wait_stops_before_sleep_and_on_shutdown_during_sleep",
];
for (const testCase of requested) {
  assert.ok(cases.includes(testCase), `Unknown lifecycle case ${testCase}`);
}
for (const testCase of requested.length ? requested : cases) {
  const name = `features::remote_bridge::tests::r46_lifecycle_regressions::${testCase}`;
  for (let attempt = 1; attempt <= 3; attempt++) {
    const run = spawnSync(executable, ["--exact", name, "--nocapture"], {
      encoding: "utf8",
      timeout: 12_000,
      windowsHide: true,
    });
    process.stdout.write(`\n${testCase} [${attempt}/3]\n${run.stdout ?? ""}${run.stderr ?? ""}`);
    if (run.error) throw run.error;
    assert.equal(run.status, 0, name);
    assert.match(run.stdout, /test result: ok\. 1 passed; 0 failed;/, name);
  }
}
