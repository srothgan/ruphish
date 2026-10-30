#!/bin/sh
# Ruphish per-user installer. No application data is created or removed here.
# Progress presentation inspired by claude-code-rust (Apache-2.0), independently implemented.

# A single final invocation keeps a partial piped download from starting installation.
ruphish_install() (
  set -eu
  umask 077
  version='' previews=false allow_downgrade=false no_path=false uninstall=false
  install_dir="${HOME:?HOME must be set}/.local/share/ruphish-install"
  tmp='' lock='' reclaim='' download_pid=''
  cyan='' magenta='' green='' yellow='' red='' reset='' live=false
  if [ -t 2 ] && [ "${TERM:-dumb}" != dumb ] && [ -z "${CI:-}" ]; then live=true; fi
  if [ -t 2 ] && [ "${TERM:-dumb}" != dumb ] && [ -z "${NO_COLOR:-}" ]; then
    cyan=$(printf '\033[36m'); magenta=$(printf '\033[35m'); green=$(printf '\033[32m')
    yellow=$(printf '\033[33m'); red=$(printf '\033[31m'); reset=$(printf '\033[0m')
  fi
  info() { printf '%s%s%s\n' "$cyan" "$*" "$reset" >&2; }
  ok() { printf '%s%s%s\n' "$green" "$*" "$reset" >&2; }
  warn() { printf '%sWARNING: %s%s\n' "$yellow" "$*" "$reset" >&2; }
  fail() { printf '%sERROR: %s%s\n' "$red" "$*" "$reset" >&2; exit 1; }
  usage() {
    cat <<'USAGE'
Usage: install.sh [options]
  --include-previews       Include dev previews when selecting the latest version.
  --version VERSION       Install this exact published version (optional leading v).
  --install-dir DIRECTORY  Managed program directory; defaults to ~/.local/share/ruphish-install.
  --no-modify-path         Do not change shell profiles.
  --allow-downgrade        Explicitly accept a program-only downgrade, including unattended runs.
  --uninstall             Remove managed program files; retain all application data.
  --help                  Show this help.
USAGE
  }
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --include-previews) previews=true ;;
      --version|--install-dir)
        option=$1; shift; [ "$#" -gt 0 ] && [ -n "$1" ] || fail "Missing value for $option"
        case "$option" in --version) version=${1#v} ;; --install-dir) install_dir=$1 ;; esac ;;
      --no-modify-path) no_path=true ;;
      --allow-downgrade) allow_downgrade=true ;;
      --uninstall) uninstall=true ;;
      --help|-h) usage; exit 0 ;;
      *) fail "Unknown option: $1 (use --help)" ;;
    esac
    shift
  done
  if "$uninstall" && { [ -n "$version" ] || "$previews" || "$allow_downgrade"; }; then
    fail '--uninstall cannot be combined with version selection or --allow-downgrade'
  fi
  if [ -n "$version" ] && "$previews"; then fail 'Choose --version or --include-previews'; fi
  valid_version() {
    case "$1" in ''|*[!0-9a-z.-]*) return 1 ;; esac
    printf '%s\n' "$1" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-dev\.[1-9][0-9]*)?$'
  }
  # Compare decimal strings without floating-point rounding or shell integer limits.
  compare_versions() {
    awk -v left="$1" -v right="$2" 'function fields(v,a, n,b) {
      n=split(v,b,/[-.]/); a[1]=b[1];a[2]=b[2];a[3]=b[3];
      a[4]=(n==3?1:0);a[5]=(n==3?0:b[5]);
    } BEGIN {fields(left,a);fields(right,b);for(i=1;i<=5;i++) {
      if(length(a[i])!=length(b[i])) {print (length(a[i])>length(b[i])?1:-1);exit}
      if("x" a[i]!="x" b[i]) {print (("x" a[i])>("x" b[i])?1:-1);exit}
    } print 0}'
  }
  [ -z "$version" ] || valid_version "$version" || fail 'Invalid version; use X.Y.Z or X.Y.Z-dev.N'
  for command in awk grep sed tar curl mktemp readlink; do command -v "$command" >/dev/null 2>&1 || fail "Required tool missing: $command"; done
  if command -v sha256sum >/dev/null 2>&1; then hash_tool=sha256sum
  elif command -v shasum >/dev/null 2>&1; then hash_tool=shasum
  else fail 'Install sha256sum or shasum before running this installer'; fi
  platform=$(uname -s); arch=$(uname -m)
  case "$platform:$arch" in
    Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
    Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu ;;
    Darwin:arm64) target=aarch64-apple-darwin ;;
    *) fail "Unsupported platform: $platform $arch. Use native Windows PowerShell on Windows; macOS requires Apple Silicon." ;;
  esac
  if ! "$uninstall"; then
    case "$platform" in
      Linux)
        libc=$(getconf GNU_LIBC_VERSION 2>/dev/null || true)
        case "$libc" in 'glibc '*) ;; *) fail 'Linux releases require glibc 2.36 or newer (musl/Alpine is unsupported)' ;; esac
        libc=${libc#glibc }
        awk -v v="$libc" 'BEGIN {split(v,a,".");exit !(a[1]>2 || (a[1]==2 && a[2]>=36))}' || fail 'Linux releases require glibc 2.36 or newer' ;;
      Darwin) [ "$(sw_vers -productVersion | cut -d. -f1)" -ge 15 ] || fail 'macOS 15 or newer is required' ;;
    esac
  fi
  case "$install_dir" in /*) ;; *) fail '--install-dir must be an absolute path' ;; esac
  case "$install_dir" in *"
"*|*"'"*) fail 'Installation paths cannot contain newlines or single quotes' ;; esac
  [ ! -L "$install_dir" ] || fail 'The installation directory must not be a symbolic link'
  parent=$(dirname "$install_dir"); leaf=$(basename "$install_dir")
  case "$leaf" in ''|.|..) fail 'Choose a dedicated program directory' ;; esac
  mkdir -p "$parent"
  install_dir="$(cd "$parent" && pwd -P)/$leaf"
  case "$install_dir" in "$HOME"|/|"$HOME/.local/share/ruphish"|"${XDG_DATA_HOME:-$HOME/.local/share}/ruphish"|"${XDG_CONFIG_HOME:-$HOME/.config}/ruphish"|"${XDG_STATE_HOME:-$HOME/.local/state}/ruphish"|"$HOME/Library/Application Support/ruphish"|"$HOME/Library/Logs/ruphish") fail 'Choose a dedicated program directory, separate from application data' ;; esac
  bin_dir="$HOME/.local/bin"; command_path="$bin_dir/ruphish"
  owned_root() {
    [ -d "$install_dir" ] && [ ! -L "$install_dir/.owner" ] && [ -f "$install_dir/.owner" ] && [ "$(cat "$install_dir/.owner")" = 'ruphish-installer-v1' ] || fail "Unmanaged installation directory: $install_dir"
    for entry in "$install_dir"/* "$install_dir"/.[!.]* "$install_dir"/..?*; do
      [ -e "$entry" ] || [ -L "$entry" ] || continue
      [ ! -L "$entry" ] || fail "Unexpected symbolic link: $entry"
      case "${entry##*/}" in .owner|.profile|current|previous|staging) ;; *) fail "Unmanaged file in installation directory: $entry" ;; esac
    done
  }
  check_package() {
    [ -d "$1" ] && [ ! -L "$1" ] || fail "Invalid package directory: $1"
    for entry in "$1"/* "$1"/.[!.]* "$1"/..?*; do
      [ -e "$entry" ] || [ -L "$entry" ] || continue
      [ -f "$entry" ] && [ ! -L "$entry" ] || fail "Unexpected package entry: $entry"
      case "${entry##*/}" in ruphish|LICENSE|THIRD-PARTY-NOTICES.md|THIRD-PARTY-NOTICES.txt|build.json|.version) ;; *) fail "Unmanaged package file: $entry" ;; esac
    done
  }
  remove_package() {
    [ -e "$1" ] || return 0
    check_package "$1"
    rm -f "$1/ruphish" "$1/LICENSE" "$1/THIRD-PARTY-NOTICES.md" "$1/THIRD-PARTY-NOTICES.txt" "$1/build.json" "$1/.version"
    rmdir "$1"
  }
  cleanup() {
    if [ -n "$download_pid" ]; then kill "$download_pid" 2>/dev/null || true; wait "$download_pid" 2>/dev/null || true; fi
    if "$live"; then printf '\r\033[2K' >&2; fi
    if [ -n "$tmp" ]; then rm -rf "$tmp"; fi
    if [ -n "$lock" ]; then rm -f "$lock/pid"; rmdir "$lock" || true; fi
    if [ -n "$reclaim" ]; then rmdir "$reclaim" || true; fi
  }
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM HUP
  lock_path="$install_dir.lock"
  if ! mkdir "$lock_path" 2>/dev/null; then
    # Serialize stale-lock recovery so two claimants cannot remove each other's new lock.
    mkdir "$lock_path.reclaim" 2>/dev/null || fail "Another installer is inspecting the lock; if it was interrupted, inspect $lock_path.reclaim"
    reclaim="$lock_path.reclaim"
    [ ! -L "$lock_path" ] && [ ! -L "$lock_path/pid" ] && [ -f "$lock_path/pid" ] || fail "Installation lock needs inspection: $lock_path"
    pid=$(cat "$lock_path/pid")
    case "$pid" in ''|*[!0-9]*) fail "Invalid installation lock: $lock_path" ;; esac
    if kill -0 "$pid" 2>/dev/null; then fail 'Another installer is running'; fi
    rm "$lock_path/pid"; rmdir "$lock_path" || fail 'Unrecognized files in installation lock'
    mkdir "$lock_path" || fail 'Another installer acquired the lock'
  fi
  lock=$lock_path; printf '%s\n' "$$" > "$lock/pid"
  if [ -n "$reclaim" ]; then rmdir "$reclaim"; reclaim=''; fi
  if [ -e "$install_dir" ]; then owned_root
  elif "$uninstall"; then info 'No managed installation found'; exit 0
  else mkdir "$install_dir"; printf 'ruphish-installer-v1\n' > "$install_dir/.owner"; fi
  # A stopped rename is recoverable without guessing which database has been used.
  if [ ! -e "$install_dir/current" ] && [ -d "$install_dir/previous" ]; then
    check_package "$install_dir/previous"; mv "$install_dir/previous" "$install_dir/current"
    warn 'Recovered program files from an interrupted replacement'
  fi
  remove_package "$install_dir/staging"
  if [ -d "$install_dir/current" ]; then
    check_package "$install_dir/current"
    [ -f "$install_dir/current/.version" ] || fail 'Installed version record is missing'
    installed=$(cat "$install_dir/current/.version"); valid_version "$installed" || fail 'Installed version record is invalid'
  else installed=''; fi
  remove_package "$install_dir/previous"
  profile_block() {
    printf '%s\n' '# >>> ruphish installer >>>' 'case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) export PATH="$HOME/.local/bin:$PATH" ;; esac' '# <<< ruphish installer <<<'
  }
  remove_profile() {
    [ -f "$install_dir/.profile" ] || return 0
    profile=$(cat "$install_dir/.profile")
    case "$profile" in "$HOME/.profile"|"$HOME/.bashrc"|"$HOME/.zshrc") ;; *) fail 'Unrecognized managed shell profile' ;; esac
    if [ -f "$profile" ]; then
      [ ! -L "$profile" ] || fail 'Refusing to edit a symbolic-link shell profile'
      # Only remove the exact block we own; preserve edited blocks for manual review.
      profile_block > "$tmp/block"
      awk 'NR==FNR {b[++n]=$0;next} {a[++m]=$0} END {for(i=1;i<=m;i++) {same=1;for(j=1;j<=n;j++) if(a[i+j-1]!=b[j]) same=0;if(same)i+=n-1;else print a[i]}}' "$tmp/block" "$profile" > "$tmp/profile"
      cat "$tmp/profile" > "$profile"
    fi
    rm "$install_dir/.profile"
  }
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/ruphish-install.XXXXXXXX")
  if "$uninstall"; then
    if [ -L "$command_path" ] && [ "$(readlink "$command_path")" = "$install_dir/current/ruphish" ]; then rm "$command_path"; fi
    if ! "$no_path"; then remove_profile; fi
    remove_package "$install_dir/current"
    if "$no_path" && [ -f "$install_dir/.profile" ]; then warn 'Shell profile left unchanged by --no-modify-path'; rm "$install_dir/.profile"; fi
    rm "$install_dir/.owner"; rmdir "$install_dir"
    ok 'Removed Ruphish program files. Configuration, databases, certificates, and logs were retained.'
    exit 0
  fi
  # Refuse command collisions before downloading or replacing files.
  if [ -e "$command_path" ] || [ -L "$command_path" ]; then
    [ -L "$command_path" ] && [ "$(readlink "$command_path")" = "$install_dir/current/ruphish" ] || fail "Another command owns $command_path"
  fi
  download() {
    url=$1; destination=$2; show=${3:-false}
    info "Downloading ${url##*/}"
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
      --retry 2 --connect-timeout 15 --max-time 600 --speed-limit 1024 --speed-time 30 \
      --dump-header "$tmp/headers" --output "$destination" "$url" 2> "$tmp/download-error" &
    download_pid=$!
    if "$live" && "$show"; then
      while kill -0 "$download_pid" 2>/dev/null; do
        bytes=0; [ ! -f "$destination" ] || bytes=$(wc -c < "$destination")
        total=$(awk 'tolower($1) ~ /^http\// {n=0} tolower($1)=="content-length:" {gsub("\r","",$2);n=$2} END {print n+0}' "$tmp/headers" 2>/dev/null || printf 0)
        bar=$(awk -v n="$bytes" -v total="$total" 'BEGIN {if(total>0) {p=int(n*100/total);if(p>100)p=100;s="";for(i=0;i<20;i++)s=s (i<int(p/5)?"=":"-");printf "[%s] %3d%%",s,p} else printf "%d bytes",n}')
        printf '\r\033[2K%sDownloading%s %s%s%s' "$magenta" "$reset" "$cyan" "$bar" "$reset" >&2
        sleep 0.2
      done
    fi
    status=0; wait "$download_pid" || status=$?; download_pid=''
    if "$live" && "$show"; then printf '\r\033[2K' >&2; fi
    if [ "$status" -ne 0 ]; then
      cat "$tmp/download-error" >&2
      case "$url" in https://ruphish.com/api/releases*) info 'If no full release is published yet, use --include-previews. You can also select an exact published --version.' ;; esac
      fail "Download failed: $url. The installed program was not replaced."
    fi
  }
  if [ -z "$version" ]; then
    info 'Resolving latest published release'
    download "https://ruphish.com/api/releases?format=text&includePreviews=$previews" "$tmp/version"
    version=$(cat "$tmp/version")
    valid_version "$version" || fail 'The release selector returned an invalid version'
    if ! "$previews"; then case "$version" in *-*) fail 'The full-release selector returned a preview' ;; esac; fi
  fi
  info "Selected Ruphish $version ($target)"
  case "$version" in *-*) warn 'Preview release: use disposable data; upgrades are not guaranteed' ;; esac
  comparison=0
  if [ -n "$installed" ]; then comparison=$(compare_versions "$version" "$installed") || fail 'Could not compare installed and selected versions'; fi
  if [ "$comparison" -lt 0 ]; then
    warn "DOWNGRADE: $installed -> $version. Only program files will change. Database migrations and configuration changes are NOT reversed."
    if ! "$allow_downgrade"; then
      if [ -z "${CI:-}" ] && [ -t 2 ] && ( : </dev/tty ) 2>/dev/null; then
        printf 'Stop is the default. Proceed with this downgrade? [y/N] ' >/dev/tty
        answer=''; read -r answer </dev/tty || true
        case "$answer" in y|Y|yes|YES) ;; *) fail 'Downgrade cancelled; existing installation retained' ;; esac
      else fail 'Downgrade requires confirmation. For an intentional unattended downgrade use --allow-downgrade'; fi
    fi
  fi
  if [ "$version" != "$installed" ]; then
    name="ruphish-$version-$target"; archive="$name.tar.gz"
    base="https://github.com/srothgan/ruphish/releases/download/v$version"
    download "$base/$archive" "$tmp/archive" true
    download "$base/SHA256SUMS" "$tmp/checksums"
    info 'Verifying archive checksum'
    expected=$(awk -v name="$archive" '$2==name {if(NF!=2 || length($1)!=64 || $1 ~ /[^0-9a-fA-F]/) bad=1;n++;hash=tolower($1)} END {if(n!=1 || bad) exit 1;print hash}' "$tmp/checksums") || fail 'Expected exactly one valid checksum for the archive'
    if [ "$hash_tool" = sha256sum ]; then actual=$(sha256sum "$tmp/archive" | awk '{print $1}')
    else actual=$(shasum -a 256 "$tmp/archive" | awk '{print $1}'); fi
    [ "$expected" = "$actual" ] || fail 'Archive checksum mismatch; existing installation retained'
    # An exact flat file allowlist also rejects traversal, duplicate names, and links.
    tar -tzf "$tmp/archive" > "$tmp/names" || fail 'Invalid tar archive'
    tar -tvzf "$tmp/archive" > "$tmp/types" || fail 'Invalid tar archive'
    awk -v root="$name" '
      {if($0==root "/") {if(seen[$0]++)exit 1;next}
       if(index($0,root "/")!=1)exit 1;f=substr($0,length(root)+2);
       if(f!="ruphish" && f!="LICENSE" && f!="THIRD-PARTY-NOTICES.md" && f!="THIRD-PARTY-NOTICES.txt" && f!="build.json")exit 1;
       if(seen[f]++)exit 1;count++} END {if(count!=5)exit 1}' "$tmp/names" || fail 'Unexpected archive paths or contents'
    awk 'substr($0,1,1)!="-" && substr($0,1,1)!="d" {exit 1}' "$tmp/types" || fail 'Archive links and special files are not allowed'
    mkdir "$tmp/extracted"
    tar -xzf "$tmp/archive" -C "$tmp/extracted"
    check_package "$tmp/extracted/$name"
    mkdir "$install_dir/staging"
    cp "$tmp/extracted/$name/"* "$install_dir/staging/"
    chmod 755 "$install_dir/staging/ruphish"
    info 'Verifying staged executable'
    reported=$("$install_dir/staging/ruphish" --version 2>&1) || fail "Staged executable could not run: $reported. Check OS/runtime requirements."
    [ "$reported" = "Ruphish $version" ] || fail "Unexpected executable version: $reported"
    printf '%s\n' "$version" > "$install_dir/staging/.version"
    if [ -d "$install_dir/current" ]; then mv "$install_dir/current" "$install_dir/previous"; fi
    if ! mv "$install_dir/staging" "$install_dir/current"; then
      [ ! -d "$install_dir/previous" ] || mv "$install_dir/previous" "$install_dir/current"
      fail 'Program replacement failed; close Ruphish and retry'
    fi
    ok "Installed Ruphish $version"
    remove_package "$install_dir/previous"
  else
    reported=$("$install_dir/current/ruphish" --version 2>&1) || fail 'The installed executable could not run'
    [ "$reported" = "Ruphish $version" ] || fail 'Installed executable and version record disagree'
    ok "Ruphish $version is already installed"
  fi
  info "Program files are ready at $install_dir/current/ruphish"
  mkdir -p "$bin_dir"
  [ -L "$command_path" ] || ln -s "$install_dir/current/ruphish" "$command_path"
  if ! "$no_path"; then
    case ":$PATH:" in *":$bin_dir:"*) ;; *)
      case "${SHELL:-sh}" in */zsh) profile="$HOME/.zshrc" ;; */bash) profile="$HOME/.bashrc" ;; */fish) profile=''; warn "Add $bin_dir to your fish PATH manually" ;; *) profile="$HOME/.profile" ;; esac
      if [ -n "$profile" ]; then
        [ ! -L "$profile" ] || fail "Program installed, but PATH setup refused a symbolic-link profile: $profile"
        if [ ! -f "$profile" ] || ! grep -Fq '# >>> ruphish installer >>>' "$profile"; then
          printf '\n' >> "$profile"; profile_block >> "$profile"
          printf '%s\n' "$profile" > "$install_dir/.profile"
        fi
      fi ;;
    esac
  fi
  found=$(command -v ruphish || true)
  if [ -n "$found" ] && [ "$found" != "$command_path" ]; then warn "Another ruphish is first on PATH: $found"; fi
  printf "\nStart now: '%s/current/ruphish'\nIn a new shell with PATH configured: ruphish\n" "$install_dir" >&2
  info 'The installer has not started the server. Ruphish creates its own configuration and data on first launch.'
)
ruphish_install "$@"
