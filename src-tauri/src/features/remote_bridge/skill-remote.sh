# Fixed, bundled operations for a single allowlisted Skill projection.
# Rust validates every interpolated value before this script is sent.
set -eu
fail() { printf '{"error":"%s"}\n' "$1"; exit 0; }
[ "$(uname -s)" = Linux ] || fail remoteUnsupported
[ "$(id -u)" != 0 ] || fail rootForbidden
for utility in base64 sha256sum stat grep cut mkdir mv rm chmod find wc awk sync sed flock; do
  command -v "$utility" >/dev/null 2>&1 || fail dependencyMissing
done
safe_component() {
  case "$1" in ''|.|..|.proxyenv-*|*[!A-Za-z0-9._-]*) return 1;; esac
  [ "${#1}" -le 96 ]
}
safe_component "$tool" || fail invalidRequest
safe_component "$skill_name" || fail invalidRequest
[ "${#skill_hash}" = 64 ] || fail invalidRequest
case "$skill_hash" in *[!0-9A-Fa-f]*) fail invalidRequest;; esac
case "$tool" in
  codex) agent_root="$HOME/.codex";;
  claude) agent_root="$HOME/.claude";;
  *) fail invalidRequest;;
esac
is_owned_directory() {
  [ -d "$1" ] && [ ! -L "$1" ] || return 1
  [ "$(stat -c %u "$1")" = "$(id -u)" ] || return 1
  mode=$(stat -c %a "$1")
  [ $((0$mode & 002)) -eq 0 ]
}
safe_directory() { [ ! -e "$1" ] || is_owned_directory "$1" || fail unsafePath; }
is_regular_owned() {
  [ -f "$1" ] && [ ! -L "$1" ] || return 1
  [ "$(stat -c %u "$1")" = "$(id -u)" ] || return 1
  [ "$(stat -c %h "$1")" = 1 ] || return 1
  mode=$(stat -c %a "$1")
  [ $((0$mode & 002)) -eq 0 ]
}
owner_matches() {
  marker="$1/.proxyenv-owner"
  is_regular_owned "$marker" || return 1
  [ "$(stat -c %s "$marker")" -le 512 ] || return 1
  [ "$(wc -l < "$marker" | awk '{print $1}')" -eq 4 ] || return 1
  [ "$(sed -n '1p' "$marker")" = proxyenv-skill-v1 ] || return 1
  [ "$(sed -n '2p' "$marker")" = "$tool" ] || return 1
  [ "$(sed -n '3p' "$marker")" = "$skill_name" ] || return 1
}
owned_hash() { sed -n '4p' "$1/.proxyenv-owner"; }
validate_tree() {
  tree="$1"
  is_owned_directory "$tree" || return 1
  owner_matches "$tree" || return 1
  manifest="$tree/.proxyenv-manifest"
  is_regular_owned "$manifest" || return 1
  [ "$(stat -c %s "$manifest")" -le 393216 ] || return 1
  [ "$(owned_hash "$tree")" = "$skill_hash" ] || return 1
  count=0
  bytes=0
  while IFS= read -r line; do
    digest=${line%%  *}
    relative=${line#*  }
    [ "${#digest}" = 64 ] || return 1
    case "$digest" in *[!0-9A-Fa-f]*) return 1;; esac
    [ "$relative" != "$line" ] || return 1
    old_ifs=$IFS; IFS='/'; set -- $relative; IFS=$old_ifs
    [ "$#" -gt 0 ] || return 1
    for component in "$@"; do safe_component "$component" || return 1; done
    file="$tree/$relative"
    is_regular_owned "$file" || return 1
    [ "$(stat -c %s "$file")" -le 2097152 ] || return 1
    [ "$(sha256sum "$file" | cut -d ' ' -f 1)" = "$digest" ] || return 1
    count=$((count + 1))
    bytes=$((bytes + $(stat -c %s "$file")))
  done < "$manifest"
  [ "$count" -eq "$file_count" ] || return 1
  [ "$bytes" -eq "$total_size" ] || return 1
  actual_files=$(find -P "$tree" -type f | wc -l | awk '{print $1}')
  [ "$actual_files" -eq $((file_count + 2)) ] || return 1
  [ -z "$(find -P "$tree" -type l -print -quit)" ] || return 1
  [ -z "$(find -P "$tree" -type d ! -user "$(id -u)" -print -quit)" ] || return 1
  [ -z "$(find -P "$tree" -type d -perm -0002 -print -quit)" ] || return 1
}
validate_existing() {
  tree="$1"
  is_owned_directory "$tree" || return 1
  owner_matches "$tree" || return 1
  manifest="$tree/.proxyenv-manifest"
  is_regular_owned "$manifest" || return 1
  [ "$(stat -c %s "$manifest")" -le 393216 ] || return 1
  count=0
  bytes=0
  while IFS= read -r line; do
    digest=${line%%  *}
    relative=${line#*  }
    [ "${#digest}" = 64 ] || return 1
    case "$digest" in *[!0-9A-Fa-f]*) return 1;; esac
    [ "$relative" != "$line" ] || return 1
    old_ifs=$IFS; IFS='/'; set -- $relative; IFS=$old_ifs
    [ "$#" -gt 0 ] || return 1
    for component in "$@"; do safe_component "$component" || return 1; done
    file="$tree/$relative"
    is_regular_owned "$file" || return 1
    size=$(stat -c %s "$file")
    [ "$size" -le 2097152 ] || return 1
    [ "$(sha256sum "$file" | cut -d ' ' -f 1)" = "$digest" ] || return 1
    count=$((count + 1))
    bytes=$((bytes + size))
    [ "$count" -le 256 ] || return 1
    [ "$bytes" -le 8388608 ] || return 1
  done < "$manifest"
  [ "$count" -gt 0 ] || return 1
  actual_files=$(find -P "$tree" -type f | wc -l | awk '{print $1}')
  [ "$actual_files" -eq $((count + 2)) ] || return 1
  [ -z "$(find -P "$tree" -type l -print -quit)" ] || return 1
  [ -z "$(find -P "$tree" -type d ! -user "$(id -u)" -print -quit)" ] || return 1
  [ -z "$(find -P "$tree" -type d -perm -0002 -print -quit)" ] || return 1
}
remove_owned_tree() {
  tree="$1"
  owner_matches "$tree" || fail skillConflict
  [ -z "$(find -P "$tree" -type l -print -quit)" ] || fail unsafePath
  find -P "$tree" -mindepth 1 ! -user "$(id -u)" -print -quit | grep -q . && fail unsafePath
  rm -rf -- "$tree" || fail remoteFailed
}

proxyenv_root="$HOME/.proxyenv"
staging_root="$proxyenv_root/staging"
skills_staging="$staging_root/skills"
tool_staging="$skills_staging/$tool"
skill_staging="$tool_staging/$skill_name"
stage="$skill_staging/$skill_hash"
skills_root="$agent_root/skills"
destination="$skills_root/$skill_name"

for directory in "$HOME" "$proxyenv_root" "$staging_root" "$skills_staging" "$tool_staging" "$skill_staging" "$agent_root" "$skills_root"; do
  safe_directory "$directory"
done
[ -d "$proxyenv_root" ] || mkdir -m 700 "$proxyenv_root" || fail unsafePath
safe_directory "$proxyenv_root"
skills_lock="$proxyenv_root/skills.lock"
[ ! -e "$skills_lock" ] || is_regular_owned "$skills_lock" || fail unsafePath
exec 9>"$skills_lock" || fail remoteFailed
chmod 600 "$skills_lock" || fail remoteFailed
flock -n 9 || fail skillBusy

case "$operation" in
  status)
    if [ ! -e "$destination" ]; then
      printf '{"state":"notSynced"}\n'
    elif ! owner_matches "$destination"; then
      printf '{"state":"conflict"}\n'
    else
      remote_hash=$(owned_hash "$destination")
      case "$remote_hash" in *[!0-9A-Fa-f]*|'') fail skillConflict;; esac
      if [ "$remote_hash" = "$skill_hash" ] && validate_tree "$destination"; then
        printf '{"state":"synced","remoteHash":"%s"}\n' "$remote_hash"
      elif validate_existing "$destination"; then
        printf '{"state":"localChanged","remoteHash":"%s"}\n' "$remote_hash"
      else
        printf '{"state":"conflict"}\n'
      fi
    fi
    ;;
  prepare)
    for directory in "$proxyenv_root" "$staging_root" "$skills_staging" "$tool_staging" "$skill_staging"; do
      [ -d "$directory" ] || mkdir -m 700 "$directory" || fail unsafePath
      safe_directory "$directory"
    done
    if [ -e "$stage" ]; then remove_owned_tree "$stage"; fi
    mkdir -m 700 "$stage" || fail remoteFailed
    printf 'proxyenv-skill-v1\n%s\n%s\n%s\n' "$tool" "$skill_name" "$skill_hash" > "$stage/.proxyenv-owner" || fail remoteFailed
    printf '%s' "$manifest_b64" | base64 -d > "$stage/.proxyenv-manifest" || fail invalidRequest
    chmod 600 "$stage/.proxyenv-owner" "$stage/.proxyenv-manifest" || fail remoteFailed
    if [ -n "$directories_b64" ]; then
      printf '%s' "$directories_b64" | base64 -d | while IFS= read -r relative; do
        [ -n "$relative" ] || continue
        old_ifs=$IFS; IFS='/'; set -- $relative; IFS=$old_ifs
        current="$stage"
        for component in "$@"; do
          safe_component "$component" || exit 20
          current="$current/$component"
          [ -d "$current" ] || mkdir -m 700 "$current" || exit 21
          safe_directory "$current"
        done
      done || fail invalidRequest
    fi
    printf '{"prepared":true}\n'
    ;;
  apply)
    validate_tree "$stage" || fail skillVerifyFailed
    [ -d "$agent_root" ] || mkdir -m 700 "$agent_root" || fail unsafePath
    safe_directory "$agent_root"
    [ -d "$skills_root" ] || mkdir -m 700 "$skills_root" || fail unsafePath
    safe_directory "$skills_root"
    backup="$skills_root/.${skill_name}.proxyenv-backup-${skill_hash}"
    safe_directory "$backup"
    [ ! -e "$backup" ] || remove_owned_tree "$backup"
    if [ -e "$destination" ]; then
      validate_existing "$destination" || fail skillConflict
      mv "$destination" "$backup" || fail remoteFailed
    fi
    if ! mv "$stage" "$destination"; then
      [ ! -e "$backup" ] || mv "$backup" "$destination" || fail rollbackFailed
      fail writeRolledBack
    fi
    if ! validate_tree "$destination"; then
      remove_owned_tree "$destination"
      [ ! -e "$backup" ] || mv "$backup" "$destination" || fail rollbackFailed
      fail writeRolledBack
    fi
    [ ! -e "$backup" ] || remove_owned_tree "$backup"
    sync -f "$skills_root" >/dev/null 2>&1 || :
    printf '{"state":"synced","remoteHash":"%s"}\n' "$skill_hash"
    ;;
  remove)
    if [ ! -e "$destination" ]; then
      printf '{"state":"notSynced"}\n'
      exit 0
    fi
    validate_existing "$destination" || fail skillConflict
    remove_owned_tree "$destination"
    printf '{"state":"notSynced"}\n'
    ;;
  cleanup)
    [ ! -e "$stage" ] || remove_owned_tree "$stage"
    printf '{"cleaned":true}\n'
    ;;
  *) fail invalidRequest;;
esac
