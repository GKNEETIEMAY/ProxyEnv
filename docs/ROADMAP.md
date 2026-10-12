# RouteSpan roadmap / 路线图

English · [简体中文](ROADMAP.zh-CN.md)

## Previous stable baseline

ProxyEnv v0.1.4 is the maintainer-designated legacy-brand stable baseline.
Its GitHub Release remains Draft; latest public release is v0.1.3.
This designation does not make a draft publicly downloadable.

## Next: RouteSpan v0.2.0

Unreleased on `v0.2.0dev`: Local Environment, Remote Bridge, CC Switch AI route
extension, tool readiness, managed Skills, Overview/Advanced and RouteSpan branding
are implemented. Acceptance evidence is separate in [release QA](RELEASE_QA_V0.2.md).

## Release gates

- Resolve the high-severity frontend dependency audit; rerun full latest-code CI.
- Verify predecessor → RouteSpan upgrade, retained installer identity/data and uninstall.
- Test real Windows 10/11, 125%/150% native DPI and desktop/tray/installer icon caches.
- Complete the live SSH key/Agent/password/KBI, provider, CLI/plugin and shared-host matrix.
- Verify reconnect, explicit disconnect, crash recovery and owned-only restore.
- Validate formal signed release workflow, updater failure/downgrade handling,
  checksums and externally verified artifact attestation.
- Review the published Markdown rendering and record remaining cross-machine preview results.

No release date is promised. Local automated checks do not replace those gates.

## Future / research

MCP bridging, additional adapters and platform expansion require separate designs.
MCP needs capability-scoped authorization, lifecycle and rollback analysis; neither
a generic forwarding tunnel nor current Skills synchronization implements it.
These are research directions, not shipped features or commitments.
