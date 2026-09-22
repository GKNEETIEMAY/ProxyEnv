# Skills remote projection — design

**Status: pending.** Model/Profile/Route synchronization exists; Skills are not yet copied to remote Codex or Claude. CC Switch may manage local Skills, but ProxyEnv must read the **target Agent's final effective Skill directory**, not CC Switch's UI enabled flag or SQLite schema. ProxyEnv does not need administrator rights merely because CC Switch may require them for local installation.

## Transaction

Inspect the local Skill directory and manifest; reject links, traversal, oversized or unsupported content; compute content hashes and sizes. Stage through OpenSSH `scp`/`sftp` in an account-owned bounded directory such as `~/.proxyenv/staging/skills/<tool>/<skill>/<hash>/`. Verify uploaded bytes and remote ownership before activation. An existing foreign Skill with the same name is a conflict, never an overwrite. A ProxyEnv-owned Skill may be updated using a verified marker and atomic replacement; disabling removes only verified owned content. Failed/interrupted transactions retain recoverable evidence and must not remove user files.

Cross-tool mapping is explicit: local Codex Skills project only to a reviewed Codex destination, and Claude Skills only to a reviewed Claude destination. Do not assume the directory format is interoperable or that an unrelated remote Skill is identical because its name matches.

## Observation and acceptance

While bridge and projection are enabled, watch lightweight local metadata, debounce changes and hash actual content before uploading. Never run `scp -r` every two seconds. Test no-op, local change, same-name foreign conflict, shared-host permissions, symlink escape, interrupted upload, ownership restore and a real remote Agent discovering the Skill. Do not claim that a copied directory is usable until the remote tool reads it.
