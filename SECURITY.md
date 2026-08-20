# Security Policy

Please report suspected vulnerabilities privately to the repository maintainers rather than opening a public issue.

ProxyEnv must only modify the current user's environment (`HKCU\\Environment`). It must snapshot values before deletion, verify every update, probe localhost candidates only, and never collect or upload proxy subscriptions, credentials, tokens, or traffic data.
