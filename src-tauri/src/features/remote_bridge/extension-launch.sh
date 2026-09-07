# Reuse installed Node; never install or change VS Code Server.
set -eu
unset NODE_OPTIONS NODE_PATH
bridge_node=''
for candidate in /usr/bin/node /usr/local/bin/node "$HOME"/.vscode-server/cli/servers/Stable-*/server/node "$HOME"/.vscode-server/bin/*/node; do
  [ -f "$candidate" ] && [ -x "$candidate" ] && [ ! -L "$candidate" ] || continue
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
  [ "$bridge_safe" = yes ] || continue
  bridge_version=$(timeout 4 "$candidate" --version 2>/dev/null) || continue
  case "$bridge_version" in v[2-9][0-9].*) ;; *) continue;; esac
  bridge_node="$candidate"
  break
done
if [ -z "$bridge_node" ]; then printf '{"error":"extensionUnsupported"}\n'; exit 0; fi
