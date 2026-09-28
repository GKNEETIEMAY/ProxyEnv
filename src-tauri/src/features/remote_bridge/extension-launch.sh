# Reuse installed Node; never install or change VS Code Server.
set -eu
unset NODE_OPTIONS NODE_PATH
bridge_node=''
bridge_node_error=remoteNodeMissing
bridge_agent_root="$HOME/.vscode-server"
case "${VSCODE_AGENT_FOLDER:-}" in "$HOME"/*) bridge_agent_root="$VSCODE_AGENT_FOLDER";; esac
for candidate in /usr/bin/node /usr/local/bin/node "$(command -v node || :)" "$HOME"/.local/bin/node "$HOME"/.nvm/versions/node/*/bin/node "$HOME"/.vscode-server/cli/servers/Stable-*/server/node "$HOME"/.vscode-server/bin/*/node "$HOME"/.vscode-server-insiders/cli/servers/Insiders-*/server/node "$HOME"/.vscode-server-insiders/bin/*/node "$HOME"/.vscode-remote/bin/*/node "$bridge_agent_root"/cli/servers/*/server/node "$bridge_agent_root"/bin/*/node; do
  [ -f "$candidate" ] && [ -x "$candidate" ] || continue
  # Resolve normal package-manager links (including /bin -> /usr/bin), then
  # validate every directory and the final executable before executing it.
  candidate=$(readlink -f -- "$candidate") || continue
  case "$candidate" in /*) ;; *) continue;; esac
  bridge_safe=yes
  bridge_parent="$candidate"
  while [ "$bridge_parent" != / ]; do
    [ ! -L "$bridge_parent" ] || bridge_safe=no
    bridge_mode=$(stat -c %a "$bridge_parent") || bridge_safe=no
    [ $((0$bridge_mode & 022)) -eq 0 ] || bridge_safe=no
    bridge_owner=$(stat -c %u "$bridge_parent") || bridge_safe=no
    [ "$bridge_owner" = 0 ] || [ "$bridge_owner" = "$(id -u)" ] || bridge_safe=no
    bridge_parent=$(dirname "$bridge_parent")
  done
  if [ "$bridge_safe" != yes ]; then bridge_node_error=remoteNodeUnsafe; continue; fi
  bridge_version=$(timeout 4 "$candidate" --version 2>/dev/null) || continue
  case "$bridge_version" in v[2-9][0-9].*) ;; *) bridge_node_error=remoteNodeUnsupported; continue;; esac
  bridge_node="$candidate"
  break
done
if [ -z "$bridge_node" ]; then printf '{"error":"%s"}\n' "$bridge_node_error"; exit 0; fi
