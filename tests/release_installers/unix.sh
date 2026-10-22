#!/usr/bin/env bash
set -euo pipefail
mode=$1 repository=$2 work=$3
trap 'echo "Unix assertion failed at line $LINENO" >&2; if [[ -f $work/calls ]]; then cat "$work/calls" >&2; fi' ERR
source "$repository/install.sh"

if [ "$mode" = progress ]; then
    curl() { printf '%s\n' "$@" > "$work/curl-arguments"; return "${curl_status:-0}"; }
    download_https 'https://example.invalid/package' "$work/package" progress
    for option in --progress-bar --fail --show-error --location --connect-timeout --max-time --max-redirs; do
        grep -qx -- "$option" "$work/curl-arguments"
    done
    [[ $(grep -c '^=https$' "$work/curl-arguments") == 2 ]]
    grep -qx 900 "$work/curl-arguments"
    grep -qx -- "$work/package" "$work/curl-arguments"
    download_https 'https://example.invalid/metadata' "$work/metadata"
    if grep -qx -- --progress-bar "$work/curl-arguments"; then echo 'Metadata enabled package progress'; exit 1; fi
    curl_status=22
    if download_https 'https://example.invalid/package' "$work/package" progress; then
        echo 'curl failure was swallowed'; exit 1
    else [[ $? == 22 ]]; fi
    echo 'Unix package progress options, metadata isolation and failure propagation passed'
    exit 0
fi

if [ "$mode" = unit ]; then
    count=0
    for fixture in "$work"/*.json; do
        for platform in linux macos; do
            if result=$(select_release_asset "$fixture" "$platform" 2>/dev/null); then
                [[ $(basename "$fixture") == valid* ]] || { echo "Unexpected acceptance: $fixture"; exit 1; }
                [[ $result == *$'\n2.3.4\n'* ]] || exit 1
            else
                [[ $(basename "$fixture") == invalid* ]] || { echo "Unexpected rejection: $fixture"; exit 1; }
            fi
            count=$((count + 1))
        done
    done
    if download_https 'http://example.invalid' "$work/unsafe" >/dev/null 2>&1; then
        echo 'HTTP transfer unexpectedly succeeded'; exit 1
    fi
    echo "Unix metadata branches: $count plus real curl HTTP rejection passed"
    exit 0
fi

if [ "$mode" = integration ]; then
    download_https 'https://api.github.com/repos/buckyos/bucky-vpn/releases/latest' "$work/unix-live.json"
    for platform in linux macos; do
        select_release_asset "$work/unix-live.json" "$platform" > "$work/fields"
        { read -r name; read -r version; read -r url; read -r digest; } < "$work/fields"
        # Windows helper has already verified all three real assets. Download the DEB
        # again with real curl to cover the second production transfer implementation.
        if [ "$platform" = linux ]; then
            download_https "$url" "$work/unix-live.deb"
            actual=$(sha256sum "$work/unix-live.deb"); actual=${actual%% *}
            [ "$actual" = "$digest" ] || exit 1
            echo "Unix production curl verified $name $actual"
        fi
    done
    exit 0
fi

[ "$mode" = dv ] || exit 1
# Rewrite only OS external paths and the installed-app filesystem boundary in a
# disposable copy. Real Bash control flow, jq, hashing, find and cleanup remain.
python3 - "$repository/install.sh" "$work" <<'PY'
from pathlib import Path
import sys
source = Path(sys.argv[1]).read_text()
work = Path(sys.argv[2])
source = source.replace('/usr/bin/xattr', str(work / 'xattr'))
source = source.replace('/usr/sbin/installer', str(work / 'installer'))
source = source.replace('/Applications/BuckyVPN.app', str(work / 'BuckyVPN.app'))
(work / 'isolated.sh').write_text(source)
PY
mkdir -p "$work/BuckyVPN.app/Contents" "$work/outside"
printf safe > "$work/BuckyVPN.app/Contents/program"
printf outside > "$work/outside/untouched"
ln -s "$work/outside" "$work/BuckyVPN.app/Contents/external-link"
cat > "$work/xattr" <<'STUB'
#!/usr/bin/env bash
printf 'xattr:%s\n' "$*" >> "$WORK/calls"
[[ $1 == -s ]] || exit 90
if [[ $SCENARIO == quarantinefail || ( $SCENARIO == appquarantinefail && $* == *BuckyVPN.app* ) ]]; then exit 13; fi
if [[ $2 != -d && $SCENARIO != noattrs ]]; then echo com.apple.quarantine; fi
STUB
cat > "$work/installer" <<'STUB'
#!/usr/bin/env bash
printf 'installer:%s\n' "$*" >> "$WORK/calls"
    if [[ $SCENARIO == nativefail || $SCENARIO == nativecleanupfail ]]; then exit 7; fi
STUB
chmod +x "$work/xattr" "$work/installer"
cat > "$work/driver.sh" <<'DRIVER'
#!/usr/bin/env bash
source "$WORK/isolated.sh"
sudo() { "$@"; }
uname() {
    if [[ $1 == -s ]]; then
        if [[ $TEST_PLATFORM == linux ]]; then echo Linux; else echo Darwin; fi
    elif [[ $SCENARIO == unsupported ]]; then echo aarch64
    else echo x86_64; fi
}
command() {
    if [[ $SCENARIO == missingjq && $2 == jq ]]; then return 1; fi
    builtin command "$@"
}
mktemp() { local directory; directory=$(/usr/bin/mktemp "$@") || return; printf '%s' "$directory" > "$WORK/created"; printf '%s\n' "$directory"; }
curl() {
    local destination='' url=${!#} show_progress=false
    while (($#)); do
        if [[ $1 == --output ]]; then destination=$2; shift; fi
        if [[ $1 == --progress-bar ]]; then show_progress=true; fi
        shift
    done
    if [[ $url == */releases/latest ]]; then [[ $show_progress == false ]] || return 91
    else [[ $show_progress == true ]] || return 91; fi
    printf 'curl:%s\n' "$url" >> "$WORK/calls"
    if [[ $SCENARIO == network ]]; then return 22; fi
    if [[ $SCENARIO == signal ]]; then kill -TERM "$BASHPID"; return 143; fi
    if [[ $url == */releases/latest ]]; then
        if [[ $SCENARIO == metadata ]]; then cp "$WORK/invalid-duplicates.json" "$destination"
        else cp "$WORK/valid.json" "$destination"; fi
    elif [[ $SCENARIO == mismatch ]]; then printf corrupt > "$destination"
    else printf package > "$destination"; fi
}
dpkg() {
    if [[ $1 == --print-architecture ]]; then echo amd64; return; fi
    printf 'dpkg:%s\n' "$*" >> "$WORK/calls"
    if [[ $SCENARIO == nativefail || $SCENARIO == nativecleanupfail ]]; then return 7; fi
}
rm() {
    if [[ $SCENARIO == cleanupfail || $SCENARIO == nativecleanupfail ]]; then return 1; fi
    /bin/rm "$@"
}
main
exit $?
DRIVER

export WORK="$work"
for TEST_PLATFORM in linux macos; do
    export TEST_PLATFORM
    for SCENARIO in success nativefail nativecleanupfail mismatch metadata network missingjq cleanupfail signal noattrs unsupported quarantinefail appquarantinefail appsymlink; do
        export SCENARIO
        # Linux has no xattr stage and x86_64 rejection is exercised there.
        [[ $TEST_PLATFORM == macos && $SCENARIO == unsupported ]] && continue
        [[ $TEST_PLATFORM == linux && ( $SCENARIO == quarantinefail || $SCENARIO == appquarantinefail || $SCENARIO == appsymlink ) ]] && continue
        if [[ $SCENARIO == appsymlink ]]; then
            mv "$work/BuckyVPN.app" "$work/saved-app"
            ln -s "$work/outside" "$work/BuckyVPN.app"
        fi
        : > "$work/calls"
        /bin/rm -f "$work/created"
        expected=1
        case "$SCENARIO" in success|noattrs) expected=0 ;; nativefail|nativecleanupfail) expected=7 ;; network) expected=22 ;; signal) expected=143 ;; metadata) expected=5 ;; quarantinefail|appquarantinefail) expected=13 ;; esac
        if bash "$work/driver.sh" > "$work/output" 2>&1; then actual=0; else actual=$?; fi
        if [[ $SCENARIO == appsymlink ]]; then
            /bin/rm "$work/BuckyVPN.app"
            mv "$work/saved-app" "$work/BuckyVPN.app"
        fi
        if [[ $actual != "$expected" ]]; then cat "$work/output"; echo "$TEST_PLATFORM/$SCENARIO returned $actual expected $expected"; exit 1; fi
        case "$SCENARIO" in
            success|noattrs|cleanupfail|nativefail|nativecleanupfail|appquarantinefail|appsymlink) grep -Eq '^(dpkg|installer):' "$work/calls" ;;
            *) if grep -Eq '^(dpkg|installer):' "$work/calls"; then echo "Unsafe launch: $SCENARIO"; exit 1; fi ;;
        esac
        if [[ $SCENARIO == success || $SCENARIO == noattrs ]]; then
            grep -q 'installed successfully' "$work/output"
        else
            if grep -q 'installed successfully' "$work/output"; then echo 'Failure reported success'; exit 1; fi
        fi
        if [[ -f $work/created ]]; then
            directory=$(cat "$work/created")
            if [[ $SCENARIO == cleanupfail || $SCENARIO == nativecleanupfail ]]; then [ -d "$directory" ]; /bin/rm -rf -- "$directory"
            else [ ! -e "$directory" ] || { echo "Leaked $directory"; exit 1; }; fi
        fi
        if [[ $TEST_PLATFORM == linux && $SCENARIO == success ]]; then grep -q '^dpkg:-i ' "$work/calls"; fi
        if [[ $TEST_PLATFORM == macos && $SCENARIO == success ]]; then
            grep -q '^installer:-allowUntrusted -pkg .* -target /$' "$work/calls"
            grep -q 'xattr:-s -d com.apple.quarantine' "$work/calls"
            # Symlinks may have their own attribute inspected; their target must not be traversed.
            if grep -q 'outside/untouched' "$work/calls"; then echo 'Followed external app symlink'; exit 1; fi
            if grep -Eq 'xattr:-s -c |Gatekeeper|spctl' "$work/calls"; then exit 1; fi
        fi
        if [[ $TEST_PLATFORM == macos && $SCENARIO == nativefail ]]; then
            if grep -q 'xattr:.*BuckyVPN.app' "$work/calls"; then echo 'App mutated after native failure'; exit 1; fi
        fi
        if [[ $TEST_PLATFORM == macos && $SCENARIO == noattrs ]]; then
            if grep -q 'xattr:-s -d' "$work/calls"; then echo 'Removed absent attributes'; exit 1; fi
        fi
        if [[ $SCENARIO == appquarantinefail || $SCENARIO == appsymlink ]]; then
            grep -q 'Package installed, but' "$work/output"
        fi
        echo "Unix full workflow passed: $TEST_PLATFORM/$SCENARIO ($actual)"
    done
done
