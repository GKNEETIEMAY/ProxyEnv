# Fixed, bundled POSIX shell operations; stdin is never an interactive terminal.
# Rust supplies only allowlisted operation/tool, validated ports and SHA-256.
set -eu
fail() { printf '{"error":"%s"}\n' "$1"; exit 0; }
[ "$(uname -s)" = Linux ] || fail remoteUnsupported
[ "$(id -u)" != 0 ] || fail rootForbidden
for utility in ss awk sha256sum mktemp flock sync cmp stat sed grep cut cp mv cat unlink; do
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
  test)
    check_ports
    command -v curl >/dev/null 2>&1 || fail dependencyMissing
    curl --disable --silent --fail --output /dev/null --max-time 12 --noproxy '' --proxy "$scheme://127.0.0.1:$port" https://www.gstatic.com/generate_204 >/dev/null 2>&1 || fail networkFailed
    printf '{"tested":true}\n'; exit 0;;
esac
umask 077
safe() {
  [ ! -L "$1" ] || fail unsafePath
  if [ -e "$1" ]; then
    [ "$(stat -c %u "$1")" = "$(id -u)" ] || fail unsafePath
    mode=$(stat -c %a "$1")
    [ $((0$mode & 022)) -eq 0 ] || fail unsafePath
    if [ -f "$1" ]; then
      [ "$(stat -c %h "$1")" = 1 ] || fail unsafePath
      [ "$(stat -c %s "$1")" -le 32768 ] || fail unsafePath
    else
      [ -d "$1" ] || fail unsafePath
    fi
  fi
}
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
if [ "$operation" = preview ] || [ "$operation" = apply ]; then
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
  printf '{"previousPort":%s,"expectedHash":"%s","version":"%s"}\n' "$previous" "$(hash "$file")" "$version"
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
  unlink "$temporary" 2>/dev/null || :
  unlink "$rollback" 2>/dev/null || :
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
if [ -f "$file" ]; then cp -p "$file" "$rollback" || fail remoteFailed; fi
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
committed=true
if [ "$operation" = restore ]; then
  [ ! -f "$backup" ] || unlink "$backup"
  unlink "$marker"
fi
printf '{"configured":%s}\n' "$([ "$operation" = apply ] && printf true || printf false)"
