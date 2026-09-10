# Fixed, bundled POSIX shell operations; stdin is never an interactive terminal.
# Rust supplies only allowlisted operation/tool, validated ports and SHA-256.
set -eu
fail() { printf '{"error":"%s"}\n' "$1"; exit 0; }
[ "$(uname -s)" = Linux ] || fail remoteUnsupported
[ "$(id -u)" != 0 ] || fail rootForbidden
for utility in ss awk sha256sum mktemp flock sync cmp stat sed grep cut cp mv cat unlink rm rmdir; do
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
if [ "$tool" = codex ]; then
  [ -z "${CODEX_HOME:-}" ] || [ "$CODEX_HOME" = "$HOME/.codex" ] || fail customHome
  directory="$HOME/.codex"
  file="$directory/proxyenv_bridge.config.toml"
else
  [ -z "${CLAUDE_CONFIG_DIR:-}" ] || fail customHome
  directory="$HOME/.claude"
  file="$directory/proxyenv-bridge.json"
fi
safe "$directory"
safe "$file"
[ ! -e "$file" ] || [ -f "$file" ] || fail unsafePath
render() {
  if [ "$tool" = codex ]; then
    printf '# ProxyEnv Remote Bridge\nmodel_provider = "proxyenv_bridge"\n\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv CC Switch"\nbase_url = "http://127.0.0.1:%s/v1"\nwire_api = "responses"\nrequires_openai_auth = false\n' "$1"
  else
    printf '{"env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:%s","ANTHROPIC_AUTH_TOKEN":"PROXY_MANAGED"}}\n' "$1"
  fi
}
hash() { if [ -f "$1" ]; then sha256sum "$1" | awk '{print $1}'; else printf absent; fi; }
render_claude_onboarding() {
  source_file="$1"
  awk '
    { text = text (NR == 1 ? "" : "\n") $0 }
    END {
      if (text == "") {
        print "{\"hasCompletedOnboarding\":true}"
        exit
      }
      first = 1
      while (first <= length(text) && substr(text, first, 1) ~ /[[:space:]]/) first++
      last = length(text)
      while (last >= first && substr(text, last, 1) ~ /[[:space:]]/) last--
      if (substr(text, first, 1) != "{" || substr(text, last, 1) != "}") exit 42
      depth = 0; square = 0; in_string = 0; escaped = 0; found = 0; value_at = 0; value_len = 0
      for (i = first; i <= last; i++) {
        ch = substr(text, i, 1)
        if (in_string) {
          if (escaped) escaped = 0
          else if (ch == "\\") escaped = 1
          else if (ch == "\"") in_string = 0
          continue
        }
        if (ch == "\"") {
          if (depth == 1 && square == 0) {
            start = i + 1; j = start; key_escaped = 0
            while (j <= last) {
              key_ch = substr(text, j, 1)
              if (key_escaped) key_escaped = 0
              else if (key_ch == "\\") key_escaped = 1
              else if (key_ch == "\"") break
              j++
            }
            if (j > last) exit 42
            key = substr(text, start, j - start)
            k = j + 1
            while (k <= last && substr(text, k, 1) ~ /[[:space:]]/) k++
            if (key == "hasCompletedOnboarding" && substr(text, k, 1) == ":") {
              found++
              if (found > 1) exit 42
              k++
              while (k <= last && substr(text, k, 1) ~ /[[:space:]]/) k++
              literal = substr(text, k, 5)
              if (substr(text, k, 4) == "true") { value_at = k; value_len = 4 }
              else if (literal == "false") { value_at = k; value_len = 5 }
              else exit 42
            }
          }
          in_string = 1
        } else if (ch == "{") depth++
        else if (ch == "}") { depth--; if (depth < 0) exit 42 }
        else if (ch == "[") square++
        else if (ch == "]") { square--; if (square < 0) exit 42 }
      }
      if (in_string || depth != 0 || square != 0) exit 42
      if (found == 1) {
        if (value_len == 4) print text
        else print substr(text, 1, value_at - 1) "true" substr(text, value_at + value_len)
        exit
      }
      inside = substr(text, first + 1, last - first - 1)
      if (inside ~ /^[[:space:]]*$/) replacement = "\n  \"hasCompletedOnboarding\": true\n"
      else replacement = "\n  \"hasCompletedOnboarding\": true," inside
      print substr(text, 1, first) replacement substr(text, last)
    }
  ' "$source_file"
}
validate() {
  if [ -f "$1" ]; then
    previous=$(sed -n 's/.*http:\/\/127\.0\.0\.1:\([0-9]*\).*/\1/p' "$1")
    case "$previous" in ''|*[!0-9]*) fail configConflict;; esac
    [ "${#previous}" -le 5 ] && [ "$previous" -ge 1024 ] && [ "$previous" -le 65535 ] || fail configConflict
    render "$previous" | cmp -s - "$1" || fail configConflict
  else
    previous=null
  fi
}
validate "$file"
if [ "$operation" = preview ] || [ "$operation" = apply ] || [ "$operation" = tool-verify ]; then
  command -v "$tool" >/dev/null 2>&1 || fail cliUnsupported
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
  if [ "$tool" = claude ]; then
    state_file="$HOME/.claude.json"
    state_preview=$(mktemp) || fail remoteFailed
    trap 'unlink "$state_preview" 2>/dev/null || :' EXIT
    if [ -f "$state_file" ]; then state_source="$state_file"; else state_source=/dev/null; fi
    if { [ ! -e "$state_file" ] || [ -f "$state_file" ]; } && is_safe "$state_file" && render_claude_onboarding "$state_source" >"$state_preview"; then
      state_hash=$(hash "$state_file")
      onboarding_managed=true
      if [ "$state_hash" = "$(hash "$state_preview")" ]; then onboarding=false; else onboarding=true; fi
    else
      # An unfamiliar Claude state must not block the independent routing
      # overlay. Leave that file untouched and surface the skipped onboarding
      # adjustment in the reviewed preview.
      onboarding_managed=false
      onboarding=false
      state_hash=unmanaged
    fi
    printf '{"previousPort":%s,"expectedHash":"%s","stateHash":"%s","onboardingRequired":%s,"onboardingManaged":%s,"version":"%s"}\n' "$previous" "$(hash "$file")" "$state_hash" "$onboarding" "$onboarding_managed" "$version"
  else
    printf '{"previousPort":%s,"expectedHash":"%s","stateHash":"absent","onboardingRequired":false,"onboardingManaged":false,"version":"%s"}\n' "$previous" "$(hash "$file")" "$version"
  fi
  exit 0
fi
if [ "$operation" = tool-verify ]; then
  [ "$tool" = claude ] || fail invalidRequest
  [ -f "$file" ] || fail configConflict
  marker="$file.proxyenv-applied"
  safe "$marker"
  [ -f "$marker" ] || fail configConflict
  [ "$(cat "$marker")" = "$(hash "$file")" ] || fail configConflict
  verify_dir=$(mktemp -d) || fail remoteFailed
  verify_out="$verify_dir/stdout"
  verify_error="$verify_dir/stderr"
  cleanup_verify() { rm -f "$verify_out" "$verify_error"; rmdir "$verify_dir" 2>/dev/null || :; }
  trap cleanup_verify EXIT
  trap 'exit 1' HUP INT TERM
  set +e
  (
    cd "$verify_dir" || exit 1
    timeout 75 claude --settings "$file" --setting-sources "" --strict-mcp-config --mcp-config '{"mcpServers":{}}' --tools "" --disallowedTools 'mcp__*' --no-session-persistence --max-turns 1 --output-format json -p 'Reply with exactly PROXYENV_VERIFY_OK.'
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
  marker="$file.proxyenv-applied"
  safe "$backup"; safe "$marker"
  [ -f "$marker" ] || fail noBackup
  [ "$(cat "$marker")" = "$(hash "$file")" ] || fail configConflict
  validate "$backup"
  printf '{"previousPort":%s,"originalPort":%s,"expectedHash":"%s","backupHash":"%s"}\n' "$before_port" "$previous" "$(hash "$file")" "$(hash "$backup")"
  exit 0
fi
[ -d "$directory" ] || mkdir -m 700 "$directory" || fail unsafePath
backup="$file.proxyenv-original"
marker="$file.proxyenv-applied"
lock="$file.proxyenv-lock"
safe "$backup"; safe "$marker"; safe "$lock"
for regular in "$backup" "$marker" "$lock"; do
  [ ! -e "$regular" ] || [ -f "$regular" ] || fail unsafePath
done
exec 9>"$lock"
flock -n 9 || fail configConflict
safe "$file"; validate "$file"
current=$(hash "$file")
had_marker=false
old_marker=''
if [ -f "$marker" ]; then had_marker=true; old_marker=$(cat "$marker"); fi
if [ "$operation" = apply ]; then
  [ "$current" = "$expected" ] || fail configConflict
  if [ -f "$marker" ]; then
    [ "$(cat "$marker")" = "$current" ] || fail configConflict
    validate "$backup"
  else
    [ ! -e "$backup" ] || fail configConflict
    if [ -f "$file" ]; then cp -p "$file" "$backup" || fail remoteFailed; fi
  fi
elif [ "$operation" = restore ]; then
  [ -f "$marker" ] || fail noBackup
  [ "$current" = "$expected" ] || fail configConflict
  [ "$(cat "$marker")" = "$current" ] || fail configConflict
  [ "$(hash "$backup")" = "$expected_backup" ] || fail configConflict
  validate "$backup"
else
  fail invalidRequest
fi
temporary=$(mktemp "$directory/.proxyenv-write.XXXXXX") || fail remoteFailed
rollback=$(mktemp "$directory/.proxyenv-rollback.XXXXXX") || fail remoteFailed
transaction=false
committed=false
next="$current"
state_transaction=false
state_changed=false
state_current=absent
state_next=absent
state_file=''
state_temporary=''
state_rollback=''
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
  if [ "$state_transaction" = true ] && [ "$committed" = false ]; then
    actual_state=$(hash "$state_file")
    if [ "$actual_state" = "$state_next" ] || [ "$actual_state" = "$state_current" ]; then
      if [ "$state_current" = absent ]; then
        [ ! -e "$state_file" ] || unlink "$state_file" || return 1
      else
        mv -f "$state_rollback" "$state_file" || return 1
      fi
      [ "$(hash "$state_file")" = "$state_current" ] || return 1
    else
      return 1
    fi
  fi
  unlink "$temporary" 2>/dev/null || :
  unlink "$rollback" 2>/dev/null || :
  [ -z "$state_temporary" ] || unlink "$state_temporary" 2>/dev/null || :
  [ -z "$state_rollback" ] || unlink "$state_rollback" 2>/dev/null || :
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
if [ -f "$file" ]; then cp -p "$file" "$rollback" || fail remoteFailed; fi
if [ "$operation" = apply ] && [ "$tool" = claude ] && [ "$expected_state" != unmanaged ]; then
  state_file="$HOME/.claude.json"
  safe "$state_file"
  [ ! -e "$state_file" ] || [ -f "$state_file" ] || fail unsafePath
  state_current=$(hash "$state_file")
  [ "$state_current" = "$expected_state" ] || fail configConflict
  state_temporary=$(mktemp "$HOME/.proxyenv-claude-state.XXXXXX") || fail remoteFailed
  state_rollback=$(mktemp "$HOME/.proxyenv-claude-rollback.XXXXXX") || fail remoteFailed
  if [ -f "$state_file" ]; then state_source="$state_file"; else state_source=/dev/null; fi
  render_claude_onboarding "$state_source" >"$state_temporary" || fail configConflict
  sync -f "$state_temporary" || fail remoteFailed
  state_next=$(hash "$state_temporary")
  if [ -f "$state_file" ]; then cp -p "$state_file" "$state_rollback" || fail remoteFailed; fi
  [ "$state_current" = "$state_next" ] || state_changed=true
fi
if [ "$operation" = apply ]; then
  render "$port" >"$temporary"
else
  if [ -f "$backup" ]; then cat "$backup" >"$temporary"; fi
fi
sync -f "$temporary" || fail remoteFailed
next=$(hash "$temporary")
safe "$file"
[ "$(hash "$file")" = "$current" ] || fail configConflict
transaction=true
# Persist recovery intent before replacing the overlay. An interrupted write
# leaves a marker; future writes fail closed instead of overwriting evidence.
if [ "$operation" = apply ]; then
  printf '%s' "$next" >"$marker"
  sync -f "$marker" || fail remoteFailed
fi
if [ "$operation" = restore ] && [ ! -f "$backup" ]; then
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
if [ "$state_changed" = true ]; then
  [ "$(hash "$state_file")" = "$state_current" ] || fail configConflict
  state_transaction=true
  mv -f "$state_temporary" "$state_file" || fail remoteFailed
  sync -f "$HOME" || :
  [ "$(hash "$state_file")" = "$state_next" ] || fail rollbackConflict
fi
committed=true
if [ "$operation" = restore ]; then
  [ ! -f "$backup" ] || unlink "$backup"
  unlink "$marker"
fi
printf '{"configured":%s}\n' "$([ "$operation" = apply ] && printf true || printf false)"
