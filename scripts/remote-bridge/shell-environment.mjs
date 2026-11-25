import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";

export const posix = p => p.replaceAll("\\", "/").replace(/^([A-Za-z]):/, (_, drive) => `/${drive.toLowerCase()}`);

export function resolveShellEnvironment() {
  const git = process.platform === "win32" ? spawnSync("where.exe",["git"],{encoding:"utf8"}).stdout?.trim().split(/\r?\n/)[0] : undefined;
  const shell = process.env.PROXYENV_TEST_SHELL || (git ? resolve(dirname(git),"../bin/bash.exe") : "/bin/sh");
  const available = existsSync(shell);
  const python = process.env.PROXYENV_TEST_PYTHON || (process.platform === "win32"
    ? spawnSync("where.exe",["python"],{encoding:"utf8"}).stdout?.trim().split(/\r?\n/)[0]
    : spawnSync("sh",["-c","command -v python3"],{encoding:"utf8"}).stdout?.trim());
  return { shell, available, python };
}

// A fixture must not source a developer's shell startup file, Python hooks,
// temporary directory or TEST_* overrides before its own script takes control.
export function createFixtureEnvironment({ home, bin, temp }, overrides = {}) {
  const isolated={...process.env};
  for(const key of Object.keys(isolated)) {
    if (/^(?:TEST_|BASH_FUNC_|MSYS)/i.test(key) || ["BASH_ENV","ENV","BASHOPTS","SHELLOPTS","CDPATH","GLOBIGNORE","PYTHONHOME","PYTHONPATH","PROXYENV_SESSION_TOKEN","extension_version"].includes(key)) delete isolated[key];
  }
  return {...isolated,CODEX_HOME:"",CLAUDE_CONFIG_DIR:"",HOME:posix(home),PATH:`${posix(bin)}:/usr/bin:/bin`,TMPDIR:posix(temp),...overrides};
}
