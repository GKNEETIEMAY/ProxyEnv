# SSH connection manager — design

**Status:** implemented on `v0.2.0dev`; release acceptance remains pending. ProxyEnv automatically imports bounded OpenSSH, VS Code Remote SSH and MobaXterm sources, and can persist manually reviewed connections. Existing structured target IDs bind source, file identity and alias/session. No private-key or credential contents are read.

## Import and manual entry

Import bounded OpenSSH `~/.ssh/config`, VS Code Remote SSH config and MobaXterm bookmark sources without scanning the disk. Do not parse Moba password, credential or master-password sections. Do not merge identically named targets from distinct sources.

Manual fields: display name, host or `user@host`, SSH port; authentication choice **Automatic OpenSSH** (default) or **specified identity file**. The default means native config, default keys, ssh-agent, password, keyboard-interactive and explicit first-use host-key confirmation—not “no authentication.” Resolve and validate host/port/identity file through the backend; store only a reviewed target definition, not secret material.

Manual connections are stored separately from model/profile settings. An identity file must be an absolute path to an existing regular, non-symlink file. ProxyEnv stores the normalized path so native OpenSSH can use it, but never reads or copies the private-key contents. Removing a manual connection removes only this local definition.

## Connection and launch

Keep native OpenSSH as protocol owner and first try noninteractive authentication. Password, key passphrase, OTP and KBI responses are transient and must not be persisted. An unknown host requires explicit fingerprint approval; an existing `known_hosts` mismatch fails. Never disable strict host-key checking or forward Agent/X11 unexpectedly.

For Moba, bookmark **import** is separate from **open**. ProxyEnv locates the actual Moba executable, fixes the source INI with `-i`, and opens the selected compatible first-level bookmark with the documented `-bookmark` argument. Nested bookmarks cannot be addressed safely through that interface and remain visible but Unsupported. Private authentication, complex jumps, macros, unsafe key conversion and opaque behavior also remain Unsupported. VS Code launches must match the selected effective SSH config; the managed terminal continues to use the existing PowerShell/OpenSSH flow.

## Verification

Automated coverage includes alias/source/path collisions, destination validation, relative or missing identity-file rejection, first-level versus nested Moba bookmarks, separate safe launch arguments, OpenSSH hardening, first-use host-key parsing and password/KBI multi-round prompts. Real-device checks for a selected identity file and installed MobaXterm remain release-acceptance items. A successful SSH check does not imply a tunnel or remote tool is ready.
