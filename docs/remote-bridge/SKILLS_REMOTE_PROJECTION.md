# Skills remote projection — design

**Status: implemented on `v0.2.0dev`; real-device Agent discovery acceptance remains pending.** Model/Profile/Route synchronization and explicit Codex/Claude Skill projection now exist. ProxyEnv reads the links that CC Switch places in the target Agent's effective Skill directory and accepts only links whose canonical target is the same-named direct child of `~/.cc-switch/skills`. Native Codex/Claude Skill folders, unrelated links and CC Switch's UI state or SQLite schema are outside the projection source.

Codex's built-in `.system` directory is deliberately excluded: those prompts are supplied and maintained by Codex, not CC Switch projection content. A missing Claude `~/.claude/skills` directory means there are no linked Claude Skills to offer; ProxyEnv neither creates that local directory nor treats its absence as an error.

## Transaction

Resolve only the top-level CC Switch link, then inspect its canonical target and manifest. Reject targets outside `~/.cc-switch/skills`, renamed aliases, nested links/reparse points, traversal, oversized or unsupported content; compute content hashes and sizes. Stage through OpenSSH `scp`/`sftp` in an account-owned bounded directory such as `~/.proxyenv/staging/skills/<tool>/<skill>/<hash>/`. Verify uploaded bytes and remote ownership before activation. An existing foreign Skill with the same name is a conflict, never an overwrite. A ProxyEnv-owned Skill may be updated using a verified marker and atomic replacement; disabling removes only verified owned content. Failed/interrupted transactions retain recoverable evidence and must not remove user files.

The implementation uploads regular files individually with native OpenSSH `scp -O`; it never performs `scp -r`. The remote transaction validates `.proxyenv-owner`, `.proxyenv-manifest`, UID ownership, modes, link count, file count, per-file/aggregate size and SHA-256 before activation. Updating or disabling also validates the previously managed tree so later third-party changes become a conflict instead of being overwritten.

Cross-tool mapping is explicit: local Codex Skills project only to a reviewed Codex destination, and Claude Skills only to a reviewed Claude destination. Do not assume the directory format is interoperable or that an unrelated remote Skill is identical because its name matches.

The connected UI groups equal Skill names into one row and shows independent Codex and Claude Code checkboxes. A discovered CC Switch link is selected for its matching remote agent by default; an explicit user opt-out is stored per tool. The same name in both tools therefore remains two separate projections even though it has one visual row.

## Observation and acceptance

While bridge and projection are enabled, watch lightweight local metadata, debounce changes and hash actual content before uploading. Never run `scp -r` every two seconds. Test no-op, local change, same-name foreign conflict, shared-host permissions, symlink escape, interrupted upload, ownership restore and a real remote Agent discovering the Skill. Do not claim that a copied directory is usable until the remote tool reads it.

The two-second monitor is active only while the bridge is connected. It checks CC Switch link presence plus enabled projection paths, sizes and modification times. New links are projected by default unless that tool-specific projection was explicitly disabled. A removed link is checked again after a short debounce, then its matching ProxyEnv-owned remote projection is removed through the same ownership-verified transaction; the other tool is unaffected. Only detected content changes trigger a full read/hash and upload. Automated boundary tests exist, but the final real Codex/Claude discovery matrix is still required before release acceptance.
