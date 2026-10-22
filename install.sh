#!/usr/bin/env bash
# Supports macOS Bash 3.2 and Debian/Ubuntu Bash. Requires curl and jq.

download_https() {
    local progress_option=''
    if [ "${3:-}" = progress ]; then progress_option=--progress-bar; fi
    curl --fail --show-error --location --proto '=https' --proto-redir '=https' \
        --connect-timeout 30 --max-time 900 --max-redirs 10 \
        --user-agent 'BuckyVPN-installer' \
        --header 'Accept: application/vnd.github+json' \
        --header 'X-GitHub-Api-Version: 2022-11-28' \
        ${progress_option:+"$progress_option"} --output "$2" "$1"
}

select_release_asset() {
    # Emit only validated fields, one per line; none can contain whitespace or shell syntax.
    jq -er --arg platform "$2" '
        if type != "object" or .draft != false or .prerelease != false
           or (.tag_name | type) != "string" or (.assets | type) != "array"
        then error("invalid latest-release metadata") else . end
        | if (.tag_name | test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$"))
          then . else error("expected a published vN.N.N release") end
        | .tag_name as $tag | ($tag | sub("^v"; "")) as $version
        | (if $platform == "linux" then "bucky-vpn_" + $version + "_amd64.deb"
           elif $platform == "macos" then "BuckyVPN-" + $version + ".pkg"
           else error("unsupported platform") end) as $name
        | [.assets[] | select(.name == $name)]
        | if length == 1 then .[0] else error("expected exactly one matching release asset") end
        | ("https://github.com/buckyos/bucky-vpn/releases/download/" + $tag + "/" + $name) as $url
        | if .state != "uploaded" or (.digest | type) != "string"
             or .browser_download_url != $url
          then error("asset is incomplete or has no official download URL/digest") else . end
        | if (.digest | test("^sha256:[0-9a-fA-F]{64}$"))
          then $name, $version, $url, (.digest | sub("^sha256:"; "" ) | ascii_downcase)
          else error("asset has no valid SHA-256 digest") end
    ' "$1"
}

run_privileged() {
    if [ "$EUID" -eq 0 ]; then "$@"; else sudo "$@"; fi
}

clear_quarantine() {
    local path="$1" attributes
    # -s changes a symlink itself, never the target outside the app tree.
    attributes=$(/usr/bin/xattr -s "$path") || return $?
    case $'\n'"$attributes"$'\n' in
        *$'\ncom.apple.quarantine\n'*)
            run_privileged /usr/bin/xattr -s -d com.apple.quarantine "$path" || return $?
            ;;
    esac
}

main() (
    # Subshell keeps options, traps and state out of callers that source this file.
    set -u
    set -o pipefail
    local temporary_directory='' platform architecture command_name asset_fields
    local asset_name version asset_url expected_digest actual_digest status path
    local os_id=''

    fail() { printf 'Bucky VPN installation failed: %s\n' "$*" >&2; }
    cleanup() {
        local original_status=$?
        trap - EXIT
        if [ -n "$temporary_directory" ] && [ -d "$temporary_directory" ]; then
            if ! rm -rf -- "$temporary_directory"; then
                printf 'Could not remove temporary downloads at %s\n' "$temporary_directory" >&2
                [ "$original_status" -ne 0 ] || original_status=1
            fi
        fi
        if [ "$original_status" -eq 0 ]; then printf 'Bucky VPN installed successfully.\n'; fi
        exit "$original_status"
    }

    if [ "$#" -ne 0 ]; then fail 'Usage: bash install.sh (no arguments)'; return 1; fi
    platform=$(uname -s) || return $?
    architecture=$(uname -m) || return $?
    case "$platform" in
        Linux)
            platform=linux
            [ "$architecture" = x86_64 ] || { fail 'Only Debian/Ubuntu amd64 is supported on Linux.'; return 1; }
            if [ ! -r /etc/os-release ]; then fail 'Cannot identify Linux distribution from /etc/os-release.'; return 1; fi
            # Read only ID, without executing the operating-system metadata as shell code.
            os_id=$(sed -n 's/^ID=//p' /etc/os-release) || return $?
            os_id=${os_id#\"}; os_id=${os_id%\"}; os_id=${os_id#\'}; os_id=${os_id%\'}
            case "$os_id" in debian|ubuntu) ;; *) fail 'Only Debian and Ubuntu are supported.'; return 1 ;; esac
            for command_name in dpkg sha256sum; do
                command -v "$command_name" >/dev/null 2>&1 || { fail "Required command missing: $command_name"; return 1; }
            done
            architecture=$(dpkg --print-architecture) || return $?
            [ "$architecture" = amd64 ] || { fail 'The dpkg architecture must be amd64.'; return 1; }
            ;;
        Darwin)
            platform=macos
            case "$architecture" in x86_64|arm64) ;; *) fail 'Only Intel and Apple Silicon macOS are supported.'; return 1 ;; esac
            for command_name in shasum /usr/sbin/installer /usr/bin/xattr /usr/bin/find; do
                command -v "$command_name" >/dev/null 2>&1 || { fail "Required command missing: $command_name"; return 1; }
            done
            ;;
        *) fail 'Supported platforms: Windows x64, Debian/Ubuntu amd64 and macOS Intel/Apple Silicon.'; return 1 ;;
    esac
    for command_name in curl jq mktemp rm; do
        command -v "$command_name" >/dev/null 2>&1 || { fail "Required command missing: $command_name"; return 1; }
    done
    if [ "$EUID" -ne 0 ]; then
        command -v sudo >/dev/null 2>&1 || { fail 'sudo is required, or run from a root session.'; return 1; }
    fi

    temporary_directory=$(mktemp -d "${TMPDIR:-/tmp}/bucky-vpn-install.XXXXXXXX") || return $?
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    trap 'exit 129' HUP
    printf 'Looking up the latest official Bucky VPN release...\n'
    download_https 'https://api.github.com/repos/buckyos/bucky-vpn/releases/latest' "$temporary_directory/release.json"
    status=$?
    if [ "$status" -ne 0 ]; then fail 'Could not fetch latest release (network, API availability or rate limit).'; return "$status"; fi
    asset_fields=$(select_release_asset "$temporary_directory/release.json" "$platform")
    status=$?
    if [ "$status" -ne 0 ]; then fail 'Cannot select a complete official release asset with SHA-256.'; return "$status"; fi
    # jq emitted four validated lines. read uses a here-document to retain variables in this shell.
    {
        IFS= read -r asset_name
        IFS= read -r version
        IFS= read -r asset_url
        IFS= read -r expected_digest
    } <<EOF
$asset_fields
EOF
    path="$temporary_directory/$asset_name"
    printf 'Downloading %s...\n' "$asset_name"
    download_https "$asset_url" "$path" progress
    status=$?
    if [ "$status" -ne 0 ]; then fail 'Package download failed.'; return "$status"; fi
    if [ "$platform" = linux ]; then actual_digest=$(sha256sum "$path"); else actual_digest=$(shasum -a 256 "$path"); fi
    status=$?
    if [ "$status" -ne 0 ]; then fail 'Could not calculate package SHA-256.'; return "$status"; fi
    actual_digest=${actual_digest%% *}
    if [ "$actual_digest" != "$expected_digest" ]; then fail 'SHA-256 mismatch. The installer will not be executed.'; return 1; fi

    printf 'Installing Bucky VPN %s...\n' "$version"
    if [ "$platform" = linux ]; then
        run_privileged dpkg -i "$path"
        status=$?
        if [ "$status" -ne 0 ]; then fail "dpkg exited with $status; inspect its output for dependency or service errors."; return "$status"; fi
    else
        clear_quarantine "$path"
        status=$?
        if [ "$status" -ne 0 ]; then fail 'Could not inspect or remove quarantine from the verified PKG.'; return "$status"; fi
        run_privileged /usr/sbin/installer -allowUntrusted -pkg "$path" -target /
        status=$?
        if [ "$status" -ne 0 ]; then fail "macOS installer exited with $status."; return "$status"; fi
        local app='/Applications/BuckyVPN.app'
        if [ ! -d "$app" ] || [ -L "$app" ]; then fail "Package installed, but $app is missing or is a symlink."; return 1; fi
        /usr/bin/find -P "$app" -print0 > "$temporary_directory/app-paths"
        status=$?
        if [ "$status" -ne 0 ]; then fail 'Package installed, but its app tree could not be inspected.'; return "$status"; fi
        while IFS= read -r -d '' path; do
            clear_quarantine "$path"
            status=$?
            if [ "$status" -ne 0 ]; then fail 'Package installed, but app quarantine handling failed.'; return "$status"; fi
        done < "$temporary_directory/app-paths"
    fi
    return 0
)

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
    exit $?
fi
