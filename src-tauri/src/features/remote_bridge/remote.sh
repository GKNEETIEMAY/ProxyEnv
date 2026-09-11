# Fixed, bundled POSIX shell operations; stdin is never an interactive terminal.
# Rust supplies only allowlisted operation/tool, validated ports and SHA-256.
set -eu
fail() { printf '{"error":"%s"}\n' "$1"; exit 0; }
[ "$(uname -s)" = Linux ] || fail remoteUnsupported
[ "$(id -u)" != 0 ] || fail rootForbidden
for utility in ss awk sha256sum mktemp flock sync stat grep cut cp mv cat unlink rm rmdir chmod; do
  command -v "$utility" >/dev/null 2>&1 || fail dependencyMissing
done
check_ports() {
  for number in $ports; do
    entries=$(ss -H -ltn "sport = :$number") || fail remoteUnsupported
    if [ "$operation" = check ]; then
      [ -z "$entries" ] || fail portInUse
    else
      [ -n "$entries" ] || fail unsafeBinding
      printf '%s\n' "$entries" | awk '{ if ($4 !~ /^127\.[0-9]+\.[0-9]+\.[0-9]+:[0-9]+$/ && $4 !~ /^\[?::1\]?:[0-9]+$/) exit 1 }' || fail unsafeBinding
    fi
  done
}
case "$operation" in
  check|verify) check_ports; printf '{"verified":true}\n'; exit 0;;
  internet)
    if ! command -v curl >/dev/null 2>&1; then
      printf '{"internet":"unknown"}\n'
    elif curl --disable --silent --fail --output /dev/null --max-time 12 --noproxy '*' https://www.gstatic.com/generate_204 >/dev/null 2>&1; then
      printf '{"internet":"reachable"}\n'
    else
      printf '{"internet":"unreachable"}\n'
    fi
    exit 0;;
  test)
    check_ports
    command -v curl >/dev/null 2>&1 || fail dependencyMissing
    curl --disable --silent --fail --output /dev/null --max-time 12 --noproxy '' --proxy "$scheme://127.0.0.1:$port" https://www.gstatic.com/generate_204 >/dev/null 2>&1 || fail networkFailed
    printf '{"tested":true}\n'; exit 0;;
esac
umask 077
is_safe() {
  [ ! -L "$1" ] || return 1
  if [ -e "$1" ]; then
    [ "$(stat -c %u "$1")" = "$(id -u)" ] || return 1
    mode=$(stat -c %a "$1")
    [ $((0$mode & 022)) -eq 0 ] || return 1
    if [ -f "$1" ]; then
      [ "$(stat -c %h "$1")" = 1 ] || return 1
      [ "$(stat -c %s "$1")" -le 32768 ] || return 1
    else
      [ -d "$1" ] || return 1
    fi
  fi
}
safe() { is_safe "$1" || fail unsafePath; }
safe "$HOME"
is_safe_user_content() {
  [ ! -L "$1" ] || return 1
  if [ -e "$1" ]; then
    [ "$(stat -c %u "$1")" = "$(id -u)" ] || return 1
    mode=$(stat -c %a "$1")
    [ $((0$mode & 002)) -eq 0 ] || return 1
    if [ $((0$mode & 020)) -ne 0 ]; then
      [ "$(stat -c %g "$1")" = "$(id -g)" ] || return 1
      [ "$(id -gn)" = "$(id -un)" ] || return 1
    fi
    if [ -f "$1" ]; then
      [ "$(stat -c %h "$1")" = 1 ] || return 1
      [ "$(stat -c %s "$1")" -le 32768 ] || return 1
    else
      [ -d "$1" ] || return 1
    fi
  fi
}
safe_user_content() { is_safe_user_content "$1" || fail unsafePath; }
is_repairable_user_content() {
  [ ! -L "$1" ] || return 1
  if [ -e "$1" ]; then
    [ "$(stat -c %u "$1")" = "$(id -u)" ] || return 1
    mode=$(stat -c %a "$1")
    [ $((0$mode & 002)) -eq 0 ] || return 1
    if [ -f "$1" ]; then
      [ "$(stat -c %h "$1")" = 1 ] || return 1
      [ "$(stat -c %s "$1")" -le 32768 ] || return 1
    else
      [ -d "$1" ] || return 1
    fi
  fi
}
# OpenSSH non-interactive sessions do not necessarily load the user's shell
# profile. CLI directories use a separate rule from configuration files:
# user-private-group write is common for NVM/npm, but world/shared-group write
# remains rejected. Shell startup files are never sourced.
is_safe_user_bin() {
  [ -d "$1" ] && [ ! -L "$1" ] || return 1
  [ "$(stat -c %u "$1")" = "$(id -u)" ] || return 1
  mode=$(stat -c %a "$1")
  [ $((0$mode & 002)) -eq 0 ] || return 1
  if [ $((0$mode & 020)) -ne 0 ]; then
    [ "$(stat -c %g "$1")" = "$(id -g)" ] || return 1
    [ "$(id -gn)" = "$(id -un)" ] || return 1
  fi
}
add_user_bin() {
  is_safe_user_bin "$1" || return 0
  case ":$PATH:" in *":$1:"*) return 0;; esac
  PATH="$PATH:$1"
}
for user_bin in \
  "$HOME/.local/bin" \
  "$HOME/.npm-global/bin" \
  "$HOME/.volta/bin" \
  "$HOME/.local/share/pnpm" \
  "$HOME/.bun/bin" \
  "$HOME/.asdf/shims" \
  "$HOME/.local/share/mise/shims" \
  "$HOME/.claude/local" \
  "$HOME/.claude/local/bin" \
  "$HOME/.claude/local/node_modules/.bin"
do
  add_user_bin "$user_bin"
done
for user_bin in "$HOME"/.nvm/versions/node/*/bin; do
  add_user_bin "$user_bin"
done
export PATH
if command -v python3 >/dev/null 2>&1; then
  config_python=python3
elif command -v python >/dev/null 2>&1 && python -c 'import sys; sys.exit(0 if sys.version_info >= (2, 7) else 1)' >/dev/null 2>&1; then
  config_python=python
else
  fail jsonEditorMissing
fi
if [ "$tool" = codex ]; then
  [ -z "${CODEX_HOME:-}" ] || [ "$CODEX_HOME" = "$HOME/.codex" ] || fail customHome
  directory="$HOME/.codex"
  file="$directory/config.toml"
else
  [ -z "${CLAUDE_CONFIG_DIR:-}" ] || fail customHome
  directory="$HOME/.claude"
  file="$directory/settings.json"
fi
permission_hardening=false
if is_safe_user_content "$directory" && is_safe_user_content "$file"; then
  :
elif is_repairable_user_content "$directory" && is_repairable_user_content "$file"; then
  permission_hardening=true
else
  fail unsafePath
fi
[ ! -e "$file" ] || [ -f "$file" ] || fail unsafePath
render() {
  if [ "$tool" = codex ]; then
    printf 'model_provider = "proxyenv_bridge"\n\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv CC Switch"\nbase_url = "http://127.0.0.1:%s/v1"\nwire_api = "responses"\nrequires_openai_auth = false\n' "$1"
  else
    printf '{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:%s","ANTHROPIC_AUTH_TOKEN":"PROXY_MANAGED"}}\n' "$1"
  fi
}
hash() { if [ -f "$1" ]; then sha256sum "$1" | awk '{print $1}'; else printf absent; fi; }
claude_json() {
  action="$1"
  source_file="$2"
  argument="${3:-0}"
  "$config_python" - "$action" "$source_file" "$argument" <<'PY'
import json
import io
import os
import re
import sys

def object_without_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate key")
        result[key] = value
    return result

action, path, argument = sys.argv[1:]
try:
    if os.path.exists(path):
        with io.open(path, "r", encoding="utf-8") as source:
            config = json.load(source, object_pairs_hook=object_without_duplicates)
    else:
        config = {}
    if not isinstance(config, dict):
        raise ValueError("root must be an object")
    env = config.get("env", {})
    if not isinstance(env, dict):
        raise ValueError("env must be an object")
except (IOError, OSError, UnicodeError, ValueError):
    sys.exit(42)

if action == "inspect":
    base_url = env.get("ANTHROPIC_BASE_URL")
    auth_token = env.get("ANTHROPIC_AUTH_TOKEN")
    other_auth = any(key in env for key in (
        "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY", "OPENAI_API_KEY"
    ))
    match = re.match(r"^http://127\.0\.0\.1:([0-9]{4,5})$", base_url or "")
    if match and auth_token == "PROXY_MANAGED" and not other_auth:
        value = int(match.group(1))
        print(value if 1024 <= value <= 65535 else "null")
    else:
        print("null")
elif action == "render":
    route_port = int(argument)
    if not 1024 <= route_port <= 65535:
        sys.exit(42)
    env = dict(env)
    env["ANTHROPIC_BASE_URL"] = "http://127.0.0.1:{0}".format(route_port)
    for key in ("ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY", "OPENAI_API_KEY"):
        env.pop(key, None)
    env["ANTHROPIC_AUTH_TOKEN"] = "PROXY_MANAGED"
    config["env"] = env
    json.dump(config, sys.stdout, ensure_ascii=True, indent=2)
    sys.stdout.write("\n")
elif action == "restore":
    backup_path = argument
    base_url = env.get("ANTHROPIC_BASE_URL")
    auth_token = env.get("ANTHROPIC_AUTH_TOKEN")
    other_auth = any(key in env for key in (
        "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY", "OPENAI_API_KEY"
    ))
    match = re.match(r"^http://127\.0\.0\.1:([0-9]{4,5})$", base_url or "")
    if not match or auth_token != "PROXY_MANAGED" or other_auth:
        sys.exit(42)
    original_exists = os.path.exists(backup_path)
    try:
        if original_exists:
            with io.open(backup_path, "r", encoding="utf-8") as source:
                original = json.load(source, object_pairs_hook=object_without_duplicates)
            if not isinstance(original, dict):
                raise ValueError("root must be an object")
            original_env = original.get("env", {})
            if not isinstance(original_env, dict):
                raise ValueError("env must be an object")
        else:
            original = {}
            original_env = {}
    except (IOError, OSError, UnicodeError, ValueError):
        sys.exit(42)
    env = dict(env)
    managed_keys = (
        "ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY", "OPENAI_API_KEY"
    )
    for key in managed_keys:
        if key in original_env:
            env[key] = original_env[key]
        else:
            env.pop(key, None)
    if env or "env" in original:
        config["env"] = env
    else:
        config.pop("env", None)
    if not original_exists and not config:
        sys.exit(0)
    json.dump(config, sys.stdout, ensure_ascii=True, indent=2)
    sys.stdout.write("\n")
else:
    sys.exit(42)
PY
}
codex_toml() {
  action="$1"
  source_file="$2"
  argument="${3:-0}"
  "$config_python" - "$action" "$source_file" "$argument" <<'PY'
from __future__ import print_function
import io
import json
import os
import re
import sys

PROVIDER = "proxyenv_bridge"
TABLE = "model_providers.proxyenv_bridge"
try:
    STRING_TYPES = (basestring,)
except NameError:
    STRING_TYPES = (str,)
HEADER = re.compile(r"^\s*\[([^\[\]]+)\]\s*(?:#.*)?$")
ARRAY_HEADER = re.compile(r"^\s*\[\[([^\[\]]+)\]\]\s*(?:#.*)?$")
ASSIGNMENT = re.compile(r"^\s*([A-Za-z0-9_.-]+)\s*=\s*(.*?)\s*$")

def fail():
    sys.exit(42)

def read_lines(path):
    if not os.path.exists(path):
        return []
    try:
        with io.open(path, "r", encoding="utf-8") as source:
            text = source.read()
    except (IOError, OSError, UnicodeError):
        fail()
    if '\x00' in text or '"""' in text or "'''" in text:
        fail()
    return text.splitlines(True)

def line_body(line):
    return line.rstrip("\r\n")

def scalar(raw):
    value = raw.strip()
    if not value:
        fail()
    quote = value[0]
    if quote not in ('"', "'"):
        token = value.split('#', 1)[0].strip()
        if token == "true":
            return True
        if token == "false":
            return False
        fail()
    escaped = False
    end = None
    for index in range(1, len(value)):
        char = value[index]
        if quote == '"' and escaped:
            escaped = False
            continue
        if quote == '"' and char == '\\':
            escaped = True
            continue
        if char == quote:
            end = index
            break
    if end is None or value[end + 1:].strip().lstrip('#').strip() and not value[end + 1:].strip().startswith('#'):
        fail()
    token = value[:end + 1]
    try:
        return json.loads(token) if quote == '"' else token[1:-1]
    except (TypeError, ValueError):
        fail()

def parse(lines):
    section = None
    top_index = None
    top_value = None
    provider_start = None
    provider_end = None
    provider_values = {}
    provider_unknown = False
    for index, line in enumerate(lines):
        body = line_body(line)
        stripped = body.strip()
        if not stripped or stripped.startswith('#'):
            continue
        header = HEADER.match(body)
        array_header = ARRAY_HEADER.match(body)
        if header or array_header:
            if provider_start is not None and provider_end is None:
                provider_end = index
            section = (header or array_header).group(1).strip()
            if section == TABLE or section.startswith(TABLE + '.'):
                if section != TABLE or provider_start is not None:
                    fail()
                provider_start = index
            continue
        assignment = ASSIGNMENT.match(body)
        if not assignment:
            if section == TABLE:
                provider_unknown = True
            continue
        key, raw = assignment.groups()
        if section is None:
            if key == "model_provider":
                if top_index is not None:
                    fail()
                top_index = index
                top_value = scalar(raw)
                if not isinstance(top_value, STRING_TYPES):
                    fail()
            elif key == "model_providers" or key.startswith(TABLE):
                fail()
        elif section == TABLE:
            if key not in ("name", "base_url", "wire_api", "requires_openai_auth") or key in provider_values:
                provider_unknown = True
            else:
                provider_values[key] = scalar(raw)
    if provider_start is not None and provider_end is None:
        provider_end = len(lines)
    route = None
    if not provider_unknown and provider_start is not None and set(provider_values) == set(("name", "base_url", "wire_api", "requires_openai_auth")):
        match = re.match(r"^http://127\.0\.0\.1:([0-9]{4,5})/v1$", provider_values.get("base_url", ""))
        if (top_value == PROVIDER and provider_values.get("name") == "ProxyEnv CC Switch"
                and provider_values.get("wire_api") == "responses"
                and isinstance(provider_values.get("requires_openai_auth"), bool) and match):
            candidate = int(match.group(1))
            if 1024 <= candidate <= 65535:
                route = candidate
    return {
        "top_index": top_index,
        "top_value": top_value,
        "provider_start": provider_start,
        "provider_end": provider_end,
        "provider_present": provider_start is not None,
        "route": route,
        "requires_openai_auth": provider_values.get("requires_openai_auth"),
    }

def canonical(port):
    return [
        "[model_providers.proxyenv_bridge]\n",
        "name = \"ProxyEnv CC Switch\"\n",
        "base_url = \"http://127.0.0.1:{0}/v1\"\n".format(port),
        "wire_api = \"responses\"\n",
        "requires_openai_auth = false\n",
    ]

def emit(lines):
    text = ''.join(lines)
    if sys.version_info[0] < 3:
        sys.stdout.write(text.encode('utf-8'))
    else:
        sys.stdout.write(text)

def without_managed(lines, parsed, top_line):
    result = []
    inserted = False
    first_table = None
    for index, line in enumerate(lines):
        body = line_body(line)
        if first_table is None and (HEADER.match(body) or ARRAY_HEADER.match(body)):
            first_table = index
        if parsed["provider_start"] is not None and parsed["provider_start"] <= index < parsed["provider_end"]:
            continue
        if index == parsed["top_index"]:
            if top_line is not None:
                result.append(top_line)
            inserted = True
            continue
        if top_line is not None and parsed["top_index"] is None and not inserted and index == first_table:
            result.append(top_line)
            inserted = True
        result.append(line)
    if top_line is not None and not inserted:
        result.append(top_line)
    return result

action, path, argument = sys.argv[1:]
lines = read_lines(path)
parsed = parse(lines)
if action == "inspect":
    print(parsed["route"] if parsed["route"] is not None else "null")
elif action == "auth-required":
    value = parsed["requires_openai_auth"]
    print("true" if value is True else "false" if value is False else "null")
elif action == "render":
    try:
        port = int(argument)
    except ValueError:
        fail()
    if not 1024 <= port <= 65535:
        fail()
    if parsed["provider_present"] and parsed["route"] is None:
        fail()
    result = without_managed(lines, parsed, 'model_provider = "proxyenv_bridge"\n')
    while result and not result[-1].strip():
        result.pop()
    if result:
        result.append("\n")
    result.extend(canonical(port))
    emit(result)
elif action == "restore":
    if parsed["route"] is None:
        fail()
    backup_lines = read_lines(argument)
    backup = parse(backup_lines)
    if backup["provider_present"]:
        fail()
    original_top = backup_lines[backup["top_index"]] if backup["top_index"] is not None else None
    result = without_managed(lines, parsed, original_top)
    while result and not result[-1].strip():
        result.pop()
    if result:
        if not result[-1].endswith(("\n", "\r")):
            result[-1] += "\n"
        result.append("\n")
    emit(result)
else:
    fail()
PY
}
validate() {
  codex_auth_required=null
  if [ -f "$1" ]; then
    if [ "$tool" = claude ]; then
      previous=$(claude_json inspect "$1") || fail configConflict
      [ "$previous" = null ] || { [ "$previous" -ge 1024 ] && [ "$previous" -le 65535 ]; } || fail configConflict
    else
      previous=$(codex_toml inspect "$1") || fail configConflict
      codex_auth_required=$(codex_toml auth-required "$1") || fail configConflict
      [ "$previous" = null ] || { [ "$previous" -ge 1024 ] && [ "$previous" -le 65535 ]; } || fail configConflict
    fi
  else
    previous=null
  fi
}
validate "$file"
marker="$file.proxyenv-applied"
safe "$marker"
marker_hash() { if [ -f "$marker" ]; then awk 'NR == 1 { print $1 }' "$marker"; else printf absent; fi; }
marker_mode() { if [ -f "$marker" ]; then awk 'NR == 1 { print $2 == "merge" ? "merge" : "exact" }' "$marker"; else printf exact; fi; }
managed_drift=false
if [ -f "$marker" ]; then
  if [ "$(marker_hash)" != "$(hash "$file")" ]; then
    [ "$previous" != null ] || fail configConflict
    managed_drift=true
  fi
  [ "$(marker_mode)" != merge ] || managed_drift=true
fi
if [ "$operation" = status ]; then
  configured=false
  if [ -f "$marker" ] && [ "$previous" != null ] && [ "$previous" = "$port" ]; then
    if [ "$tool" != codex ] || [ "$codex_auth_required" = false ]; then configured=true; fi
  fi
  printf '{"configured":%s,"previousPort":%s}\n' "$configured" "$previous"
  exit 0
fi
if [ "$operation" = preview ] || [ "$operation" = apply ] || [ "$operation" = tool-verify ]; then
  command -v "$tool" >/dev/null 2>&1 || fail cliMissing
  command -v timeout >/dev/null 2>&1 || fail dependencyMissing
  version=$(timeout 8 "$tool" --version 2>/dev/null) || fail cliUnsupported
  if [ "$tool" = codex ]; then
    printf '%s' "$version" | grep -Eq '^codex-cli 0\.[0-9]+\.[0-9]+$' || fail cliUnsupported
    version=${version#codex-cli }
    minor=$(printf '%s' "$version" | cut -d. -f2)
    [ "$minor" -ge 134 ] || fail cliUnsupported
  else
    printf '%s' "$version" | grep -Eq '^2\.[0-9]+\.[0-9]+ \(Claude Code\)$' || fail cliUnsupported
    version=${version% (Claude Code)}
  fi
fi
if [ "$operation" = preview ]; then
  if [ -f "$file" ]; then config_exists=true; else config_exists=false; fi
  printf '{"previousPort":%s,"expectedHash":"%s","configExists":%s,"permissionHardening":%s,"version":"%s"}\n' "$previous" "$(hash "$file")" "$config_exists" "$permission_hardening" "$version"
  exit 0
fi
if [ "$operation" = tool-verify ]; then
  [ "$tool" = claude ] || fail invalidRequest
  [ -f "$file" ] || fail configConflict
  [ -f "$marker" ] || fail configConflict
  validate "$file"
  [ "$previous" != null ] || fail configConflict
  [ "$previous" = "$port" ] || fail routeOutdated
  verify_dir=$(mktemp -d) || fail remoteFailed
  verify_out="$verify_dir/stdout"
  verify_error="$verify_dir/stderr"
  verify_settings="$verify_dir/settings.json"
  cleanup_verify() { rm -f "$verify_out" "$verify_error" "$verify_settings"; rmdir "$verify_dir" 2>/dev/null || :; }
  trap cleanup_verify EXIT
  trap 'exit 1' HUP INT TERM
  render "$previous" >"$verify_settings" || fail remoteFailed
  set +e
  (
    cd "$verify_dir" || exit 1
    timeout 75 claude --settings "$verify_settings" --setting-sources "" --strict-mcp-config --mcp-config '{"mcpServers":{}}' --tools "" --disallowedTools 'mcp__*' --no-session-persistence --max-turns 1 --output-format json -p 'Reply with exactly PROXYENV_VERIFY_OK.'
  ) >"$verify_out" 2>"$verify_error"
  verify_status=$?
  set -e
  if [ "$verify_status" -eq 0 ] && grep -q 'PROXYENV_VERIFY_OK' "$verify_out"; then
    verification=verified
  elif [ "$verify_status" -eq 124 ]; then
    verification=timedOut
  elif grep -Eiq 'not logged in|log in|login|authentication|authenticate|oauth|api key|auth token' "$verify_out" "$verify_error"; then
    verification=authenticationRequired
  elif grep -Eiq 'connection refused|failed to connect|network|socket|econnrefused|gateway|502|503|504' "$verify_out" "$verify_error"; then
    verification=routeUnavailable
  else
    verification=failed
  fi
  cleanup_verify
  trap - EXIT HUP INT TERM
  printf '{"verification":"%s"}\n' "$verification"
  exit 0
fi
if [ "$operation" = restore-preview ]; then
  before_port="$previous"
  backup="$file.proxyenv-original"
  safe_user_content "$backup"; safe "$marker"
  [ -f "$marker" ] || fail noBackup
  validate "$backup"
  if [ -f "$backup" ]; then original_exists=true; else original_exists=false; fi
  printf '{"previousPort":%s,"originalPort":%s,"expectedHash":"%s","backupHash":"%s","originalExists":%s,"permissionHardening":%s}\n' "$before_port" "$previous" "$(hash "$file")" "$(hash "$backup")" "$original_exists" "$permission_hardening"
  exit 0
fi
[ -d "$directory" ] || mkdir -m 700 "$directory" || fail unsafePath
backup="$file.proxyenv-original"
lock="$file.proxyenv-lock"
if [ "$permission_hardening" = true ]; then
  [ "$repair_permissions" = true ] || fail configConflict
  [ "$(hash "$file")" = "$expected" ] || fail configConflict
  chmod 700 "$directory" || fail unsafePath
  [ ! -f "$file" ] || chmod 600 "$file" || fail unsafePath
  [ ! -f "$backup" ] || chmod 600 "$backup" || fail unsafePath
  [ ! -f "$marker" ] || chmod 600 "$marker" || fail unsafePath
  [ "$(hash "$file")" = "$expected" ] || fail configConflict
fi
safe_user_content "$backup"; safe "$marker"; safe "$lock"
for regular in "$backup" "$marker" "$lock"; do
  [ ! -e "$regular" ] || [ -f "$regular" ] || fail unsafePath
done
exec 9>"$lock"
flock -n 9 || fail configConflict
safe_user_content "$file"; validate "$file"
current=$(hash "$file")
had_marker=false
old_marker=''
create_backup=false
if [ -f "$marker" ]; then had_marker=true; old_marker=$(cat "$marker"); fi
if [ "$operation" = apply ]; then
  [ "$current" = "$expected" ] || fail configConflict
  if [ -f "$marker" ]; then
    if [ "$(marker_hash)" != "$current" ]; then
      [ "$previous" != null ] || fail configConflict
      managed_drift=true
    fi
    [ "$(marker_mode)" != merge ] || managed_drift=true
    validate "$backup"
  else
    [ ! -e "$backup" ] || fail configConflict
    if [ -f "$file" ]; then create_backup=true; fi
  fi
elif [ "$operation" = restore ]; then
  [ -f "$marker" ] || fail noBackup
  [ "$current" = "$expected" ] || fail configConflict
  if [ "$(marker_hash)" != "$current" ]; then
    [ "$previous" != null ] || fail configConflict
    managed_drift=true
  fi
  [ "$(marker_mode)" != merge ] || managed_drift=true
  [ "$(hash "$backup")" = "$expected_backup" ] || fail configConflict
  validate "$backup"
else
  fail invalidRequest
fi
temporary=$(mktemp "$directory/.proxyenv-write.XXXXXX") || fail remoteFailed
rollback=$(mktemp "$directory/.proxyenv-rollback.XXXXXX") || fail remoteFailed
transaction=false
committed=false
created_backup=false
next="$current"
restore_to_absent=false
cleanup() {
  if [ "$transaction" = true ] && [ "$committed" = false ]; then
    actual=$(hash "$file")
    if [ "$actual" = "$next" ] || [ "$actual" = "$current" ]; then
      if [ "$current" = absent ]; then
        [ ! -e "$file" ] || unlink "$file" || return 1
      else
        mv -f "$rollback" "$file" || return 1
      fi
      [ "$(hash "$file")" = "$current" ] || return 1
      if [ "$had_marker" = true ]; then
        printf '%s' "$old_marker" >"$marker"
      else
        [ ! -f "$marker" ] || unlink "$marker"
        [ ! -f "$backup" ] || unlink "$backup"
      fi
    else
      # Unknown third-party state: retain backup and marker for manual recovery.
      return 1
    fi
  fi
  if [ "$transaction" = false ] && [ "$created_backup" = true ]; then
    unlink "$backup" 2>/dev/null || :
  fi
  unlink "$temporary" 2>/dev/null || :
  unlink "$rollback" 2>/dev/null || :
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
if [ -f "$file" ]; then cp -p "$file" "$rollback" || fail remoteFailed; fi
if [ "$operation" = apply ]; then
  if [ "$tool" = claude ]; then
    claude_json render "$file" "$port" >"$temporary" || fail configConflict
    rendered_port=$(claude_json inspect "$temporary") || fail configConflict
    [ "$rendered_port" = "$port" ] || fail verifyFailed
  else
    codex_toml render "$file" "$port" >"$temporary" || fail configConflict
    rendered_port=$(codex_toml inspect "$temporary") || fail configConflict
    [ "$rendered_port" = "$port" ] || fail verifyFailed
  fi
else
  if [ "$managed_drift" = true ]; then
    if [ "$tool" = claude ]; then
      claude_json restore "$file" "$backup" >"$temporary" || fail configConflict
    else
      codex_toml restore "$file" "$backup" >"$temporary" || fail configConflict
    fi
    [ -s "$temporary" ] || restore_to_absent=true
  elif [ -f "$backup" ]; then
    cat "$backup" >"$temporary"
  else
    restore_to_absent=true
  fi
fi
if [ "$create_backup" = true ]; then
  cp -p "$file" "$backup" || fail remoteFailed
  created_backup=true
  sync -f "$backup" || fail remoteFailed
fi
sync -f "$temporary" || fail remoteFailed
next=$(hash "$temporary")
safe_user_content "$file"
[ "$(hash "$file")" = "$current" ] || fail configConflict
transaction=true
# Persist recovery intent before replacing the overlay. An interrupted write
# leaves a marker; future writes fail closed instead of overwriting evidence.
if [ "$operation" = apply ]; then
  marker_kind=exact
  [ "$had_marker" = false ] || marker_kind=$(marker_mode)
  [ "$managed_drift" = false ] || marker_kind=merge
  printf '%s %s' "$next" "$marker_kind" >"$marker"
  sync -f "$marker" || fail remoteFailed
fi
if [ "$operation" = restore ] && [ "$restore_to_absent" = true ]; then
  unlink "$file" || fail remoteFailed
  next=absent
else
  mv -f "$temporary" "$file" || fail remoteFailed
fi
sync -f "$directory" || :
if [ "$(hash "$file")" != "$next" ]; then
  # A third party changed it: never clobber their edit during rollback.
  fail rollbackConflict
fi
validate "$file"
if [ "$operation" = apply ]; then
  [ "$previous" = "$port" ] || fail rollbackConflict
fi
committed=true
if [ "$operation" = restore ]; then
  [ ! -f "$backup" ] || unlink "$backup"
  unlink "$marker"
fi
if [ "$operation" = apply ]; then
  printf '{"configured":true,"appliedHash":"%s","backupHash":"%s"}\n' "$next" "$(hash "$backup")"
else
  printf '{"configured":false}\n'
fi
