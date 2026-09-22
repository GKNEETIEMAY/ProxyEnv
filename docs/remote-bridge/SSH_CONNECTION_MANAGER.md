# SSH connection manager — design

**Status:** discovery exists; manual add, general import management and reliable Moba launch are pending. Existing structured target IDs bind source, file identity and alias/session. No private-key or credential contents are read.

## Import and manual entry

Import bounded OpenSSH `~/.ssh/config`, VS Code Remote SSH config and MobaXterm bookmark sources without scanning the disk. Do not parse Moba password, credential or master-password sections. Do not merge identically named targets from distinct sources.

Manual fields: display name, host or `user@host`, SSH port; authentication choice **Automatic OpenSSH** (default) or **specified identity file**. The default means native config, default keys, ssh-agent, password, keyboard-interactive and explicit first-use host-key confirmation—not “no authentication.” Resolve and validate host/port/identity file through the backend; store only a reviewed target definition, not secret material.

## Connection and launch

Keep native OpenSSH as protocol owner and first try noninteractive authentication. Password, key passphrase, OTP and KBI responses are transient and must not be persisted. An unknown host requires explicit fingerprint approval; an existing `known_hosts` mismatch fails. Never disable strict host-key checking or forward Agent/X11 unexpectedly.

For Moba, separate bookmark **import** from **open**: locate the actual Moba executable and launch only a safely compatible SSH session with verified source identity. Private authentication, complex jumps, macros, unsafe key conversion and opaque behavior remain Unsupported. Do not say “openable” merely because the bookmark was parsed. VS Code launches must match the selected effective SSH config; the managed terminal can continue to use the existing PowerShell/OpenSSH flow.

## Verification

Test alias collisions, malformed paths, same-name Moba sessions, first-use host-key, password/KBI multi-round prompts, identity-file selection, unsupported bookmarks and safe launch arguments. A successful SSH check does not imply a tunnel or remote tool is ready.
