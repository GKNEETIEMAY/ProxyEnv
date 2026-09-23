# Fixed, bundled POSIX shell operations; stdin is never an interactive terminal.
# Rust supplies only allowlisted operation/tool, validated ports and SHA-256.
set -eu
fail() { printf '{"error":"%s"}\n' "$1"; exit 0; }
[ "$(uname -s)" = Linux ] || fail remoteUnsupported
[ "$(id -u)" != 0 ] || fail rootForbidden
for utility in ss awk sha256sum mktemp flock sync stat grep cut cp mv cat unlink rm rmdir chmod mkdir; do
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
    # Keep the session credential out of argv/process listings. curl reads the
    # proxy URL from stdin and the local relay strips credentials upstream.
    printf 'proxy = "%s://proxyenv:%s@127.0.0.1:%s"\n' "$scheme" "$session_token" "$port" |
      curl --disable --silent --fail --output /dev/null --max-time 12 --noproxy '' --config - https://www.gstatic.com/generate_204 >/dev/null 2>&1 || fail networkFailed
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

if [ "$operation" = session-env-apply ] || [ "$operation" = session-env-remove ]; then
  umask 077
  proxyenv_root="$HOME/.proxyenv"
  sessions_root="$proxyenv_root/sessions"
  safe "$proxyenv_root"
  safe "$sessions_root"
  [ -d "$proxyenv_root" ] || mkdir -m 700 "$proxyenv_root" || fail unsafePath
  [ -d "$sessions_root" ] || mkdir -m 700 "$sessions_root" || fail unsafePath
  safe "$proxyenv_root"
  safe "$sessions_root"
  session_directory="$sessions_root/$session_id"
  session_file="$session_directory/env.sh"
  session_marker="# ProxyEnv managed session $session_id"
  safe "$session_directory"
  safe "$session_file"
  if [ -e "$session_file" ]; then
    [ -f "$session_file" ] || fail configConflict
    grep -Fqx "$session_marker" "$session_file" || fail configConflict
  fi
  if [ "$operation" = session-env-remove ]; then
    [ ! -f "$session_file" ] || unlink "$session_file" || fail remoteFailed
    [ ! -d "$session_directory" ] || rmdir "$session_directory" 2>/dev/null || fail remoteFailed
    printf '{"sessionEnvironment":"removed"}\n'
    exit 0
  fi
  [ -d "$session_directory" ] || mkdir -m 700 "$session_directory" || fail unsafePath
  temporary=$(mktemp "$session_directory/.env.XXXXXX") || fail remoteFailed
  cleanup_session_environment() { unlink "$temporary" 2>/dev/null || :; }
  trap cleanup_session_environment EXIT
  {
    printf '%s\n' "$session_marker"
    printf '%s\n' 'unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY'
    case "$protocol" in
      http)
        printf "export HTTP_PROXY='http://proxyenv:%s@127.0.0.1:%s'\n" "$session_token" "$port"
        printf "export HTTPS_PROXY='http://proxyenv:%s@127.0.0.1:%s'\n" "$session_token" "$port"
        ;;
      socks5)
        printf "export ALL_PROXY='socks5h://proxyenv:%s@127.0.0.1:%s'\n" "$session_token" "$port"
        ;;
      mixed)
        printf "export HTTP_PROXY='http://proxyenv:%s@127.0.0.1:%s'\n" "$session_token" "$port"
        printf "export HTTPS_PROXY='http://proxyenv:%s@127.0.0.1:%s'\n" "$session_token" "$port"
        printf "export ALL_PROXY='socks5h://proxyenv:%s@127.0.0.1:%s'\n" "$session_token" "$port"
        ;;
      *) fail invalidRequest;;
    esac
    printf "%s\n" "export NO_PROXY='localhost,127.0.0.1,::1'"
  } >"$temporary" || fail remoteFailed
  chmod 600 "$temporary" || fail remoteFailed
  mv -f "$temporary" "$session_file" || fail remoteFailed
  sync -f "$session_file" || fail remoteFailed
  trap - EXIT
  printf '{"sessionEnvironment":"applied"}\n'
  exit 0
fi

is_safe_profile_catalog() {
  [ ! -L "$1" ] || return 1
  if [ -e "$1" ]; then
    [ -f "$1" ] || return 1
    [ "$(stat -c %u "$1")" = "$(id -u)" ] || return 1
    [ "$(stat -c %h "$1")" = 1 ] || return 1
    [ "$(stat -c %s "$1")" -le 8388608 ] || return 1
    mode=$(stat -c %a "$1")
    [ $((0$mode & 002)) -eq 0 ] || return 1
    if [ $((0$mode & 020)) -ne 0 ]; then
      [ "$(stat -c %g "$1")" = "$(id -g)" ] || return 1
      [ "$(id -gn)" = "$(id -un)" ] || return 1
    fi
  fi
}
safe_profile_catalog() { is_safe_profile_catalog "$1" || fail unsafePath; }
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
  codex_catalog="$directory/.proxyenv-bridge-model-catalog.json"
  profile_catalog="$directory/proxyenv-codex-model-catalog.json"
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
    printf 'model_provider = "proxyenv_bridge"\n\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv Local Bridge"\nbase_url = "http://127.0.0.1:%s/v1"\nwire_api = "responses"\nrequires_openai_auth = false\nsupports_websockets = false\nhttp_headers = { "X-ProxyEnv-Session" = "%s" }\n' "$1" "$session_token"
  else
    printf '{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:%s","ANTHROPIC_AUTH_TOKEN":"PROXY_MANAGED","ANTHROPIC_CUSTOM_HEADERS":"X-ProxyEnv-Session: %s"}}\n' "$1" "$session_token"
  fi
}
hash() { if [ -f "$1" ]; then sha256sum "$1" | awk '{print $1}'; else printf absent; fi; }
codex_catalog_json() {
  printf '%s\n' '{"models":[{"additional_speed_tiers":[],"availability_nux":null,"base_instructions":"You are Codex, a coding agent. You and the user share the same workspace and collaborate to achieve their goals.","context_window":131072,"default_reasoning_level":"medium","default_reasoning_summary":"none","description":"Follows the local Codex or CC Switch model through ProxyEnv.","display_name":"ProxyEnv Bridge","effective_context_window_percent":95,"experimental_supported_tools":[],"input_modalities":["text","image"],"max_context_window":131072,"priority":1000,"service_tiers":[],"shell_type":"shell_command","slug":"proxyenv-bridge","support_verbosity":false,"supported_in_api":true,"supported_reasoning_levels":[{"description":"Fast responses with lighter reasoning","effort":"low"},{"description":"Balanced reasoning for normal tasks","effort":"medium"},{"description":"Greater reasoning depth for complex problems","effort":"high"}],"supports_image_detail_original":false,"supports_parallel_tool_calls":false,"supports_reasoning_summaries":true,"supports_search_tool":false,"truncation_policy":{"limit":10000,"mode":"bytes"},"upgrade":null,"visibility":"list"}]}'
}
codex_catalog_hash() { codex_catalog_json | sha256sum | awk '{print $1}'; }
codex_catalog_matches() { [ -f "$codex_catalog" ] && [ "$(hash "$codex_catalog")" = "$(codex_catalog_hash)" ]; }
claude_json() {
  action="$1"
  source_file="$2"
  argument="${3:-0}"
  profile_file="${4:-}"
  "$config_python" - "$action" "$source_file" "$argument" "$profile_file" <<'PY'
import json
import hashlib
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

action, path, argument, profile_path = sys.argv[1:]
MODEL_KEYS = ("model", "availableModels", "modelOverrides", "effortLevel", "alwaysThinkingEnabled")
SESSION_HEADER = "X-ProxyEnv-Session"
session_token = os.environ.get("PROXYENV_SESSION_TOKEN", "")
if session_token and not re.match(r"^[0-9a-fA-F]{64}$", session_token):
    sys.exit(42)

def model_env_key(key):
    return (key in ("ANTHROPIC_MODEL", "CLAUDE_CODE_SUBAGENT_MODEL", "CLAUDE_CODE_EFFORT_LEVEL", "ANTHROPIC_CUSTOM_MODEL_OPTION")
            or any(key == "ANTHROPIC_DEFAULT_{0}_{1}".format(tier, field)
                   for tier in ("OPUS", "SONNET", "HAIKU", "FABLE")
                   for field in ("MODEL", "MODEL_NAME")))

def load_profile():
    if not profile_path:
        return {}
    try:
        with io.open(profile_path, "r", encoding="utf-8") as source:
            profile = json.load(source, object_pairs_hook=object_without_duplicates)
        if not isinstance(profile, dict) or any(key not in MODEL_KEYS + ("env",) for key in profile):
            raise ValueError("invalid profile")
        profile_env = profile.get("env", {})
        if not isinstance(profile_env, dict) or any(not model_env_key(key) for key in profile_env):
            raise ValueError("invalid profile env")
        return profile
    except (IOError, OSError, UnicodeError, ValueError, TypeError):
        sys.exit(42)
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

if action == "profile-hash":
    projected = {}
    for key in MODEL_KEYS:
        if key in config:
            projected[key] = config[key]
    projected_env = dict((key, value) for key, value in env.items() if model_env_key(key))
    if projected_env:
        projected["env"] = projected_env
    payload = json.dumps(projected, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    print(hashlib.sha256(payload).hexdigest())
elif action == "inspect":
    base_url = env.get("ANTHROPIC_BASE_URL")
    auth_token = env.get("ANTHROPIC_AUTH_TOKEN")
    custom_headers = env.get("ANTHROPIC_CUSTOM_HEADERS")
    other_auth = any(key in env for key in (
        "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY", "OPENAI_API_KEY"
    ))
    match = re.match(r"^http://127\.0\.0\.1:([0-9]{4,5})$", base_url or "")
    header_supported = (custom_headers is None or
                        custom_headers == "{0}: {1}".format(SESSION_HEADER, session_token) or
                        re.match(r"^X-ProxyEnv-Session: [0-9a-fA-F]{64}$", custom_headers or ""))
    if match and auth_token == "PROXY_MANAGED" and not other_auth and header_supported:
        value = int(match.group(1))
        print(value if 1024 <= value <= 65535 else "null")
    else:
        print("null")
elif action == "session-match":
    print("true" if env.get("ANTHROPIC_CUSTOM_HEADERS") ==
          "{0}: {1}".format(SESSION_HEADER, session_token) else "false")
elif action == "render":
    route_port = int(argument)
    if not 1024 <= route_port <= 65535:
        sys.exit(42)
    profile = load_profile()
    for key in MODEL_KEYS:
        config.pop(key, None)
    for key in MODEL_KEYS:
        if key in profile:
            config[key] = profile[key]
    env = dict(env)
    for key in list(env):
        if model_env_key(key):
            env.pop(key, None)
    env.update(profile.get("env", {}))
    env["ANTHROPIC_BASE_URL"] = "http://127.0.0.1:{0}".format(route_port)
    for key in ("ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY", "OPENAI_API_KEY"):
        env.pop(key, None)
    env["ANTHROPIC_AUTH_TOKEN"] = "PROXY_MANAGED"
    env["ANTHROPIC_CUSTOM_HEADERS"] = "{0}: {1}".format(SESSION_HEADER, session_token)
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
        "OPENROUTER_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_CUSTOM_HEADERS"
    )
    managed_keys = managed_keys + tuple(key for key in set(list(env) + list(original_env)) if model_env_key(key))
    for key in managed_keys:
        if key in original_env:
            env[key] = original_env[key]
        else:
            env.pop(key, None)
    if env or "env" in original:
        config["env"] = env
    else:
        config.pop("env", None)
    for key in MODEL_KEYS:
        if key in original:
            config[key] = original[key]
        else:
            config.pop(key, None)
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
  profile_model="${4:-}"
  "$config_python" - "$action" "$source_file" "$argument" "$profile_model" <<'PY'
from __future__ import print_function
import io
import json
import os
import re
import sys

PROVIDER = "proxyenv_bridge"
LEGACY_MODEL = "proxyenv-bridge"
LEGACY_CATALOG = ".proxyenv-bridge-model-catalog.json"
TABLE = "model_providers.proxyenv_bridge"
try:
    STRING_TYPES = (basestring,)
except NameError:
    STRING_TYPES = (str,)
HEADER = re.compile(r"^\s*\[([^\[\]]+)\]\s*(?:#.*)?$")
ARRAY_HEADER = re.compile(r"^\s*\[\[([^\[\]]+)\]\]\s*(?:#.*)?$")
ASSIGNMENT = re.compile(r"^\s*([A-Za-z0-9_.-]+)\s*=\s*(.*?)\s*$")
SESSION_HEADER = "X-ProxyEnv-Session"
SESSION_HEADERS = re.compile(r'^\{\s*"X-ProxyEnv-Session"\s*=\s*"([0-9a-fA-F]{64})"\s*\}(?:\s*#.*)?$')

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
    top_indexes = {}
    top_values = {}
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
            if key in ("model_provider", "model", "model_catalog_json"):
                if key in top_indexes:
                    fail()
                top_indexes[key] = index
                top_values[key] = scalar(raw)
                if not isinstance(top_values[key], STRING_TYPES):
                    fail()
            elif key == "model_providers" or key.startswith(TABLE):
                fail()
        elif section == TABLE:
            if key not in ("name", "base_url", "wire_api", "requires_openai_auth", "supports_websockets", "http_headers") or key in provider_values:
                provider_unknown = True
            elif key == "http_headers":
                header_match = SESSION_HEADERS.match(raw.strip())
                if header_match:
                    provider_values[key] = header_match.group(1)
                else:
                    provider_unknown = True
            else:
                provider_values[key] = scalar(raw)
    if provider_start is not None and provider_end is None:
        provider_end = len(lines)
    route = None
    provider_keys = set(provider_values)
    supported_provider = provider_keys in (
        set(("name", "base_url", "wire_api", "requires_openai_auth")),
        set(("name", "base_url", "wire_api", "requires_openai_auth", "supports_websockets")),
        set(("name", "base_url", "wire_api", "requires_openai_auth", "http_headers")),
        set(("name", "base_url", "wire_api", "requires_openai_auth", "supports_websockets", "http_headers")),
    )
    if not provider_unknown and provider_start is not None and supported_provider:
        match = re.match(r"^http://127\.0\.0\.1:([0-9]{4,5})/v1$", provider_values.get("base_url", ""))
        if (top_values.get("model_provider") == PROVIDER and provider_values.get("name") in ("ProxyEnv CC Switch", "ProxyEnv Local Bridge")
                and provider_values.get("wire_api") == "responses"
                and isinstance(provider_values.get("requires_openai_auth"), bool)
                and provider_values.get("supports_websockets") in (None, False) and match):
            candidate = int(match.group(1))
            if 1024 <= candidate <= 65535:
                route = candidate
    return {
        "top_indexes": top_indexes,
        "top_values": top_values,
        "provider_start": provider_start,
        "provider_end": provider_end,
        "provider_present": provider_start is not None,
        "route": route,
        "legacy_model": (top_values.get("model") == LEGACY_MODEL
                         and top_values.get("model_catalog_json") == LEGACY_CATALOG),
        "profile_catalog": top_values.get("model_catalog_json") == "proxyenv-codex-model-catalog.json",
        "requires_openai_auth": provider_values.get("requires_openai_auth"),
        "session_header": provider_values.get("http_headers"),
    }

def canonical(port):
    session_token = os.environ.get("PROXYENV_SESSION_TOKEN", "")
    if not re.match(r"^[0-9a-fA-F]{64}$", session_token):
        fail()
    return [
        "[model_providers.proxyenv_bridge]\n",
        "name = \"ProxyEnv Local Bridge\"\n",
        "base_url = \"http://127.0.0.1:{0}/v1\"\n".format(port),
        "wire_api = \"responses\"\n",
        "requires_openai_auth = false\n",
        "supports_websockets = false\n",
        "http_headers = {{ \"X-ProxyEnv-Session\" = {0} }}\n".format(json.dumps(session_token)),
    ]

def emit(lines):
    text = ''.join(lines)
    if sys.version_info[0] < 3:
        sys.stdout.write(text.encode('utf-8'))
    else:
        sys.stdout.write(text)

def top_line(lines, parsed, key):
    index = parsed["top_indexes"].get(key)
    return lines[index] if index is not None else None

def rewrite(lines, parsed, replacements):
    result = []
    inserted = set()
    first_table = None
    for index, line in enumerate(lines):
        body = line_body(line)
        if first_table is None and (HEADER.match(body) or ARRAY_HEADER.match(body)):
            first_table = index
        if parsed["provider_start"] is not None and parsed["provider_start"] <= index < parsed["provider_end"]:
            continue
        replaced = False
        for key, replacement in replacements.items():
            if index == parsed["top_indexes"].get(key):
                if replacement is not None:
                    result.append(replacement)
                inserted.add(key)
                replaced = True
                break
        if replaced:
            continue
        if index == first_table:
            for key, replacement in replacements.items():
                if replacement is not None and key not in inserted and key not in parsed["top_indexes"]:
                    result.append(replacement)
                    inserted.add(key)
        result.append(line)
    for key, replacement in replacements.items():
        if replacement is not None and key not in inserted and key not in parsed["top_indexes"]:
            result.append(replacement)
    return result

action, path, argument, profile_model = sys.argv[1:]
lines = read_lines(path)
parsed = parse(lines)
if action == "inspect":
    print(parsed["route"] if parsed["route"] is not None else "null")
elif action == "auth-required":
    value = parsed["requires_openai_auth"]
    print("true" if value is True else "false" if value is False else "null")
elif action == "session-match":
    print("true" if parsed["session_header"] == os.environ.get("PROXYENV_SESSION_TOKEN", "") else "false")
elif action == "legacy-model":
    print("true" if parsed["legacy_model"] else "false")
elif action == "current-model":
    value = parsed["top_values"].get("model")
    if isinstance(value, STRING_TYPES) and 0 < len(value) <= 256 and not any(ord(char) < 32 or ord(char) == 127 for char in value):
        print(json.dumps(value))
    else:
        print("null")
elif action == "render":
    try:
        port = int(argument)
    except ValueError:
        fail()
    if not 1024 <= port <= 65535:
        fail()
    if parsed["provider_present"] and parsed["route"] is None:
        fail()
    if not profile_model or len(profile_model) > 256 or any(ord(char) < 32 or ord(char) == 127 for char in profile_model):
        fail()
    replacements = {
        "model_provider": 'model_provider = "proxyenv_bridge"\n',
        "model": 'model = {0}\n'.format(json.dumps(profile_model)),
        "model_catalog_json": 'model_catalog_json = "proxyenv-codex-model-catalog.json"\n',
    }
    if parsed["legacy_model"]:
        backup_lines = read_lines(path + ".proxyenv-original")
        backup = parse(backup_lines)
        original_model = top_line(backup_lines, backup, "model")
        if original_model is None or backup["top_values"].get("model") == LEGACY_MODEL:
            sys.exit(43)
    result = rewrite(lines, parsed, replacements)
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
    replacements = {
        "model_provider": top_line(backup_lines, backup, "model_provider"),
        "model": top_line(backup_lines, backup, "model"),
        "model_catalog_json": top_line(backup_lines, backup, "model_catalog_json"),
    }
    result = rewrite(lines, parsed, replacements)
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
  codex_legacy_model=false
  codex_remote_model=null
  claude_remote_profile_hash=''
  session_current=false
  if [ -f "$1" ]; then
    if [ "$tool" = claude ]; then
      previous=$(claude_json inspect "$1") || fail configConflict
      session_current=$(claude_json session-match "$1") || fail configConflict
      claude_remote_profile_hash=$(claude_json profile-hash "$1") || fail configConflict
      [ "$previous" = null ] || { [ "$previous" -ge 1024 ] && [ "$previous" -le 65535 ]; } || fail configConflict
    else
      previous=$(codex_toml inspect "$1") || fail configConflict
      session_current=$(codex_toml session-match "$1") || fail configConflict
      codex_auth_required=$(codex_toml auth-required "$1") || fail configConflict
      codex_legacy_model=$(codex_toml legacy-model "$1") || fail configConflict
      codex_remote_model=$(codex_toml current-model "$1") || fail configConflict
      [ "$previous" = null ] || { [ "$previous" -ge 1024 ] && [ "$previous" -le 65535 ]; } || fail configConflict
    fi
  else
    previous=null
  fi
}
validate "$file"
profile_model=''
profile_catalog_temp=''
profile_settings_temp=''
if [ "$tool" = codex ] && { [ "$operation" = preview ] || [ "$operation" = apply ]; }; then
  command -v base64 >/dev/null 2>&1 || fail dependencyMissing
  profile_model=$(printf '%s' "$profile_model_b64" | base64 -d 2>/dev/null) || fail localCodexProfileInvalid
  [ -n "$profile_model" ] || fail localCodexProfileInvalid
  profile_catalog_temp=$(mktemp) || fail remoteFailed
  printf '%s' "$profile_catalog_b64" | base64 -d >"$profile_catalog_temp" 2>/dev/null || fail localCatalogUnsafe
  computed_profile_hash=$("$config_python" -c 'from __future__ import print_function
import hashlib, io, json, sys
model=sys.argv[1]
try:
 data=io.open(sys.argv[2], "rb").read()
 root=json.loads(data.decode("utf-8"))
 models=root.get("models") if isinstance(root, dict) else None
 valid=isinstance(models, list) and any(isinstance(item, dict) and (item.get("slug") == model or item.get("model") == model or item.get("id") == model) for item in models)
 if not valid: raise ValueError()
 print(hashlib.sha256(model.encode("utf-8") + b"\0" + data).hexdigest())
except Exception:
 sys.exit(42)' "$profile_model" "$profile_catalog_temp") || fail localCatalogUnsafe
  [ "$computed_profile_hash" = "$profile_hash" ] || fail localCodexProfileInvalid
fi
if [ "$tool" = claude ] && { [ "$operation" = preview ] || [ "$operation" = apply ]; }; then
  command -v base64 >/dev/null 2>&1 || fail dependencyMissing
  profile_settings_temp=$(mktemp) || fail remoteFailed
  printf '%s' "$profile_settings_b64" | base64 -d >"$profile_settings_temp" 2>/dev/null || fail localClaudeProfileInvalid
  computed_profile_hash=$(claude_json profile-hash "$profile_settings_temp") || fail localClaudeProfileInvalid
  [ "$computed_profile_hash" = "$profile_hash" ] || fail localClaudeProfileInvalid
fi
marker="$file.proxyenv-applied"
safe "$marker"
marker_hash() { if [ -f "$marker" ]; then awk 'NR == 1 { print $1 }' "$marker"; else printf absent; fi; }
marker_mode() { if [ -f "$marker" ]; then awk 'NR == 1 { print $2 == "merge" ? "merge" : "exact" }' "$marker"; else printf exact; fi; }
marker_profile_hash() { if [ -f "$marker" ]; then awk 'NR == 1 { print $3 }' "$marker"; fi; }
marker_catalog_hash() { if [ -f "$marker" ]; then awk 'NR == 1 { print $4 }' "$marker"; fi; }
if [ "$tool" = codex ]; then
  safe_user_content "$codex_catalog"
  [ ! -e "$codex_catalog" ] || [ -f "$codex_catalog" ] || fail unsafePath
  safe_profile_catalog "$profile_catalog"
  [ ! -e "$profile_catalog" ] || [ -f "$profile_catalog" ] || fail unsafePath
fi
managed_drift=false
if [ -f "$marker" ]; then
  if [ "$(marker_hash)" != "$(hash "$file")" ]; then
    managed_drift=true
  fi
  [ "$(marker_mode)" != merge ] || managed_drift=true
fi
if [ "$tool" = claude ] && [ -f "$marker" ] && [ "$(marker_profile_hash)" != "$claude_remote_profile_hash" ]; then
  managed_drift=true
fi
if [ "$operation" = status ]; then
  configured=false
  if [ -f "$marker" ] && [ "$previous" != null ] && [ "$previous" = "$port" ] && [ "$session_current" = true ]; then
    if [ "$tool" != codex ] || { [ "$codex_auth_required" = false ] && [ "$codex_legacy_model" = false ] && [ -f "$profile_catalog" ] && [ "$(hash "$profile_catalog")" = "$(marker_catalog_hash)" ]; }; then configured=true; fi
  fi
  if [ "$tool" = claude ]; then current_profile_hash="$claude_remote_profile_hash"; else current_profile_hash="$(marker_profile_hash)"; fi
  printf '{"configured":%s,"owned":%s,"previousPort":%s,"remoteModel":%s,"profileHash":"%s"}\n' "$configured" "$([ -f "$marker" ] && printf true || printf false)" "$previous" "$codex_remote_model" "$current_profile_hash"
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
    claude_minor=$(printf '%s' "$version" | cut -d. -f2)
    claude_patch=$(printf '%s' "$version" | cut -d. -f3)
    { [ "$claude_minor" -gt 1 ] || { [ "$claude_minor" -eq 1 ] && [ "$claude_patch" -ge 227 ]; }; } || fail cliUnsupported
  fi
fi
if [ "$operation" = preview ]; then
  if [ "$tool" = codex ] && [ "$codex_legacy_model" = true ]; then
    legacy_backup="$file.proxyenv-original"
    safe_user_content "$legacy_backup"
    [ -f "$legacy_backup" ] || fail legacyModelSelectionRequired
  fi
  if [ -f "$file" ]; then config_exists=true; else config_exists=false; fi
  if [ "$tool" = claude ]; then current_profile_hash="$claude_remote_profile_hash"; else current_profile_hash="$(marker_profile_hash)"; fi
  printf '{"previousPort":%s,"expectedHash":"%s","configExists":%s,"owned":%s,"permissionHardening":%s,"version":"%s","profileHash":"%s","remoteChanged":%s}\n' "$previous" "$(hash "$file")" "$config_exists" "$([ -f "$marker" ] && printf true || printf false)" "$permission_hardening" "$version" "$current_profile_hash" "$managed_drift"
  [ -z "$profile_catalog_temp" ] || unlink "$profile_catalog_temp"
  [ -z "$profile_settings_temp" ] || unlink "$profile_settings_temp"
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
legacy_extension_backup="$file.proxyenv-extension-original"
legacy_extension_state="$file.proxyenv-extension-state"
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
if [ "$tool" = codex ]; then
  safe_user_content "$legacy_extension_backup"
  safe_user_content "$legacy_extension_state"
fi
for regular in "$backup" "$marker" "$lock" "$legacy_extension_backup" "$legacy_extension_state"; do
  [ ! -e "$regular" ] || [ -f "$regular" ] || fail unsafePath
done
exec 9>"$lock"
flock -n 9 || fail configConflict
safe_user_content "$file"; validate "$file"
legacy_catalog_owned="$codex_legacy_model"
current=$(hash "$file")
had_marker=false
old_marker=''
create_backup=false
backup_source=''
legacy_extension_owned=false
if [ -f "$marker" ]; then had_marker=true; old_marker=$(cat "$marker"); fi
if [ "$tool" = codex ] && [ ! -f "$marker" ]; then
  if [ -f "$legacy_extension_state" ]; then
    legacy_backup_hash=$(hash "$legacy_extension_backup")
    "$config_python" - "$legacy_extension_state" "$current" "$legacy_backup_hash" <<'PY' || fail configConflict
from __future__ import print_function
import io
import json
import sys

try:
    STRING_TYPES = (basestring,)
except NameError:
    STRING_TYPES = (str,)
try:
    with io.open(sys.argv[1], "r", encoding="utf-8") as source:
        record = json.load(source)
    valid = (
        isinstance(record, dict)
        and record.get("schema") == 1
        and record.get("tool") == "codex"
        and record.get("state") == "applied"
        and record.get("originalHash") == sys.argv[3]
        and isinstance(record.get("appliedHash"), STRING_TYPES)
        and len(record.get("appliedHash")) == 64
        and all(char in "0123456789abcdef" for char in record.get("appliedHash"))
        and isinstance(record.get("port"), int)
        and 1024 <= record.get("port") <= 65535
    )
except (IOError, OSError, UnicodeError, ValueError, TypeError):
    valid = False
sys.exit(0 if valid else 42)
PY
    legacy_extension_owned=true
  elif [ -e "$legacy_extension_backup" ]; then
    fail configConflict
  fi
fi
if [ "$operation" = apply ]; then
  [ "$current" = "$expected" ] || fail configConflict
  if [ -f "$marker" ]; then
    if [ "$tool" = codex ] && [ "$codex_legacy_model" = true ] && [ -f "$codex_catalog" ]; then codex_catalog_matches || fail configConflict; fi
    if [ "$(marker_hash)" != "$current" ]; then
      managed_drift=true
    fi
    [ "$(marker_mode)" != merge ] || managed_drift=true
    managed_previous="$previous"
    managed_legacy_model="$codex_legacy_model"
    validate "$backup"
    previous="$managed_previous"
    codex_legacy_model="$managed_legacy_model"
  else
    [ ! -e "$backup" ] || unlink "$backup" || fail configConflict
    if [ "$legacy_extension_owned" = true ]; then
      create_backup=legacy
    elif [ -f "$file" ]; then
      create_backup=true
      backup_source="$file"
    fi
  fi
elif [ "$operation" = restore ]; then
  [ -f "$marker" ] || fail noBackup
  if [ "$tool" = codex ] && [ "$codex_legacy_model" = true ] && [ -f "$codex_catalog" ]; then codex_catalog_matches || fail configConflict; fi
  [ "$current" = "$expected" ] || fail configConflict
  if [ "$(marker_hash)" != "$current" ]; then
    managed_drift=true
  fi
  [ "$(marker_mode)" != merge ] || managed_drift=true
  [ "$(hash "$backup")" = "$expected_backup" ] || fail configConflict
  managed_previous="$previous"
  managed_legacy_model="$codex_legacy_model"
  validate "$backup"
  previous="$managed_previous"
  codex_legacy_model="$managed_legacy_model"
else
  fail invalidRequest
fi
temporary=$(mktemp "$directory/.proxyenv-write.XXXXXX") || fail remoteFailed
rollback=$(mktemp "$directory/.proxyenv-rollback.XXXXXX") || fail remoteFailed
catalog_rollback=''
legacy_baseline=''
if [ "$tool" = codex ] && [ -f "$profile_catalog" ]; then
  catalog_rollback=$(mktemp "$directory/.proxyenv-catalog-rollback.XXXXXX") || fail remoteFailed
  cp -p "$profile_catalog" "$catalog_rollback" || fail remoteFailed
fi
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
      if [ "$tool" = codex ]; then
        if [ -n "$catalog_rollback" ]; then mv -f "$catalog_rollback" "$profile_catalog" || return 1
        else [ ! -e "$profile_catalog" ] || unlink "$profile_catalog" || return 1
        fi
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
  [ -z "$profile_catalog_temp" ] || unlink "$profile_catalog_temp" 2>/dev/null || :
  [ -z "$profile_settings_temp" ] || unlink "$profile_settings_temp" 2>/dev/null || :
  [ -z "$catalog_rollback" ] || unlink "$catalog_rollback" 2>/dev/null || :
  [ -z "$legacy_baseline" ] || unlink "$legacy_baseline" 2>/dev/null || :
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
if [ -f "$file" ]; then cp -p "$file" "$rollback" || fail remoteFailed; fi
if [ "$create_backup" = legacy ]; then
  legacy_baseline=$(mktemp "$directory/.proxyenv-legacy-baseline.XXXXXX") || fail remoteFailed
  if [ "$previous" != null ]; then
    requested_profile_model="$profile_model"
    codex_toml restore "$file" "$legacy_extension_backup" >"$legacy_baseline" || fail configConflict
    profile_model="$requested_profile_model"
  elif [ -f "$file" ]; then
    cat "$file" >"$legacy_baseline" || fail remoteFailed
  fi
  if [ -s "$legacy_baseline" ]; then
    create_backup=true
    backup_source="$legacy_baseline"
  else
    create_backup=false
  fi
fi
if [ "$operation" = apply ]; then
  if [ "$tool" = claude ]; then
    claude_json render "$file" "$port" "$profile_settings_temp" >"$temporary" || fail configConflict
    rendered_port=$(claude_json inspect "$temporary") || fail configConflict
    [ "$rendered_port" = "$port" ] || fail verifyFailed
    [ "$(claude_json profile-hash "$temporary")" = "$profile_hash" ] || fail verifyFailed
  else
    set +e
    codex_toml render "$file" "$port" "$profile_model" >"$temporary"
    render_status=$?
    set -e
    [ "$render_status" -ne 43 ] || fail legacyModelSelectionRequired
    [ "$render_status" -eq 0 ] || fail configConflict
    rendered_port=$(codex_toml inspect "$temporary") || fail configConflict
    [ "$rendered_port" = "$port" ] || fail verifyFailed
  fi
else
  if [ "$managed_drift" = true ]; then
    if [ "$tool" = claude ]; then
      claude_json restore "$file" "$backup" >"$temporary" || fail configConflict
    elif [ "$previous" = null ]; then
      if [ -f "$file" ]; then cat "$file" >"$temporary"; else restore_to_absent=true; fi
    else
      set +e
      codex_toml restore "$file" "$backup" >"$temporary"
      restore_status=$?
      set -e
      [ "$restore_status" -ne 43 ] || fail legacyModelSelectionRequired
      [ "$restore_status" -eq 0 ] || fail configConflict
    fi
    [ -s "$temporary" ] || restore_to_absent=true
  elif [ -f "$backup" ]; then
    cat "$backup" >"$temporary"
  else
    restore_to_absent=true
  fi
fi
if [ "$create_backup" = true ]; then
  cp -p "$backup_source" "$backup" || fail remoteFailed
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
  if [ "$tool" = codex ]; then
    printf '%s %s %s %s' "$next" "$marker_kind" "$profile_hash" "$(hash "$profile_catalog_temp")" >"$marker"
  else
    printf '%s %s %s' "$next" "$marker_kind" "$profile_hash" >"$marker"
  fi
  sync -f "$marker" || fail remoteFailed
fi
if [ "$operation" = apply ] && [ "$tool" = codex ]; then
  mv -f "$profile_catalog_temp" "$profile_catalog" || fail remoteCatalogWriteFailed
  chmod 600 "$profile_catalog" || fail remoteCatalogWriteFailed
  sync -f "$profile_catalog" || fail remoteCatalogWriteFailed
  profile_catalog_temp=''
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
  [ "$session_current" = true ] || fail rollbackConflict
fi
if [ "$tool" = codex ] && [ "$legacy_catalog_owned" = true ] && [ -f "$codex_catalog" ]; then
  codex_catalog_matches || fail configConflict
  unlink "$codex_catalog" || fail remoteFailed
fi
committed=true
if [ "$tool" = codex ]; then
  [ ! -f "$legacy_extension_state" ] || unlink "$legacy_extension_state" 2>/dev/null || :
  [ ! -f "$legacy_extension_backup" ] || unlink "$legacy_extension_backup" 2>/dev/null || :
fi
if [ "$operation" = restore ]; then
  if [ "$tool" = codex ] && [ -f "$profile_catalog" ]; then unlink "$profile_catalog" || fail remoteCatalogWriteFailed; fi
  [ ! -f "$backup" ] || unlink "$backup"
  unlink "$marker"
fi
if [ "$operation" = apply ]; then
  printf '{"configured":true,"appliedHash":"%s","backupHash":"%s"}\n' "$next" "$(hash "$backup")"
else
  printf '{"configured":false}\n'
fi
