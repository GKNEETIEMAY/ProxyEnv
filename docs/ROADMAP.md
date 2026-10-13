# RouteSpan roadmap / 路线图

English · [简体中文](ROADMAP.zh-CN.md)

## Previous stable baseline

ProxyEnv v0.1.4 is the maintainer-designated legacy-brand stable baseline.
Its GitHub Release remains Draft; v0.1.3 was the previous public release.
This designation does not make a draft publicly downloadable.

## Released: RouteSpan v0.2.0

Published as the latest stable release on 2026-10-13: Local Environment, Remote Bridge,
CC Switch AI route extension, tool readiness, managed Skills, Overview/Advanced
and RouteSpan branding. The release gates are closed, including the production
updater smoke. [Release QA](RELEASE_QA_V0.2.md) separates current publication status
from historical checkpoints; detailed final acceptance records remain private.

## Gates retained for future releases

- Preserve the high-severity frontend audit gate and rerun full CI for release-preflight changes.
- Verify predecessor → RouteSpan upgrade, retained installer identity/data and uninstall.
- Test real Windows 10/11, 125%/150% native DPI and desktop/tray/installer icon caches.
- Complete the live SSH key/Agent/password/KBI, provider, CLI/plugin and shared-host matrix.
- Verify reconnect, explicit disconnect, crash recovery and owned-only restore.
- Validate formal signed release workflow, updater failure/downgrade handling,
  checksums and externally verified artifact attestation.
- Review published Markdown rendering and record scoped cross-machine results.

No future release date is promised. Local automated checks do not replace manual gates.

## Future / research

MCP bridging, additional adapters and platform expansion require separate designs.
MCP needs capability-scoped authorization, lifecycle and rollback analysis; neither
a generic forwarding tunnel nor current Skills synchronization implements it.
These are research directions, not shipped features or commitments.
