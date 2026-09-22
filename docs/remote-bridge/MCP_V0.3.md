# MCP remote work — v0.3 research only

**Not part of v0.2.** Local CC Switch MCP enablement does not automatically make an MCP server available to remote Codex/Claude. MCP may require executables, Node, Python, uv, credentials and network access on the server; copying configuration is neither sufficient nor safe.

For v0.3, investigate HTTP/SSE/Streamable HTTP bridging first, then stdio runtime detection and compatibility. Do not auto-install runtimes with `npm`/`pip`, copy provider secrets, rely on CC Switch database schema or convert protocol semantics. Other CC Switch-compatible tool adapters (Gemini/OpenCode/OpenClaw) also belong to later research, not v0.2 acceptance.
