#!/usr/bin/env sh
# Install symphony-runner CLI from GitHub Releases.
# Usage: curl -fsSL https://raw.githubusercontent.com/Industrial/rust-symphony/main/install.sh | sh
#
# Optional: SYMPHONY_VERSION=v1.2.3 to pin version (default: latest release)
# Optional: SYMPHONY_INSTALL_DIR=/path to set install directory (default: ~/.local/bin or /usr/local/bin for root)

set -e

REPO="Industrial/rust-symphony"
RELEASES_URL="https://github.com/${REPO}/releases"
API_URL="https://api.github.com/repos/${REPO}/releases"

# Resolve install directory
get_install_dir() {
    if [ -n "${SYMPHONY_INSTALL_DIR}" ]; then
        echo "${SYMPHONY_INSTALL_DIR}"
        return
    fi
    if [ "$(id -u)" = "0" ] 2>/dev/null; then
        echo "/usr/local/bin"
    else
        echo "${HOME}/.local/bin"
    fi
}

# Detect OS and arch; set TARGET and EXT (tar.gz or zip)
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m | tr '[:upper:]' '[:lower:]')

    case "${ARCH}" in
        x86_64|amd64) ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *) ARCH="" ;;
    esac

    case "${OS}" in
        linux)
            case "${ARCH}" in
                x86_64) TARGET="x86_64-unknown-linux-gnu"; EXT="tar.gz" ;;
                aarch64) TARGET="aarch64-unknown-linux-gnu"; EXT="tar.gz" ;;
                *) unsupported ;;
            esac
            ;;
        darwin)
            case "${ARCH}" in
                x86_64) TARGET="x86_64-apple-darwin"; EXT="tar.gz" ;;
                aarch64) TARGET="aarch64-apple-darwin"; EXT="tar.gz" ;;
                *) unsupported ;;
            esac
            ;;
        *)
            unsupported
            ;;
    esac
}

unsupported() {
    echo "Unsupported platform: $(uname -s) $(uname -m)"
    echo "Pre-built binaries are available for Linux (x86_64, aarch64) and macOS (x86_64, arm64)."
    echo "See ${RELEASES_URL} for manual download, or install with: cargo install symphony-runner"
    exit 1
}

# Get latest release tag (without 'v' prefix for URL) or use SYMPHONY_VERSION
get_version() {
    if [ -n "${SYMPHONY_VERSION}" ]; then
        # Strip leading 'v' if present
        echo "${SYMPHONY_VERSION}" | sed 's/^v//'
        return
    fi
    # Fetch latest tag from GitHub API (no jq required); strip leading 'v'
    RAW=$(curl -sSf "${API_URL}/latest" | grep '"tag_name":' | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/' | head -1)
    TAG=$(echo "${RAW}" | sed 's/^v//')
    if [ -z "${TAG}" ]; then
        echo "Could not determine latest release. Set SYMPHONY_VERSION or check ${RELEASES_URL}"
        exit 1
    fi
    echo "${TAG}"
}

main() {
    echo "Installing symphony-runner..."
    detect_platform
    VERSION=$(get_version)
    INSTALL_DIR=$(get_install_dir)
    # Release assets use tag with 'v' in the URL
    TAG_V="v${VERSION}"
    ARCHIVE="symphony-runner-${TAG_V}-${TARGET}.${EXT}"
    DOWNLOAD_URL="${RELEASES_URL}/download/${TAG_V}/${ARCHIVE}"

    mkdir -p "${INSTALL_DIR}"
    tmpdir=$(mktemp -d)
    trap "rm -rf ${tmpdir}" EXIT

    echo "Downloading ${DOWNLOAD_URL} ..."
    if ! curl -fsSL "${DOWNLOAD_URL}" -o "${tmpdir}/${ARCHIVE}"; then
        echo "Download failed. The release may not have binaries for this platform yet."
        echo "See ${RELEASES_URL}"
        exit 1
    fi

    cd "${tmpdir}"
    if [ "${EXT}" = "tar.gz" ]; then
        tar -xzf "${ARCHIVE}"
    else
        unzip -q "${ARCHIVE}"
    fi

    # Binary is named 'symphony'
    if [ -f "symphony" ]; then
        chmod +x symphony
        mv symphony "${INSTALL_DIR}/symphony"
    else
        echo "Archive did not contain expected 'symphony' binary."
        exit 1
    fi

    echo "Installed symphony to ${INSTALL_DIR}/symphony"
    if ! echo "${PATH}" | grep -q "${INSTALL_DIR}"; then
        echo ""
        echo "Add symphony to your PATH:"
        echo " export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo "Add the above to your shell profile (~/.bashrc, ~/.zshrc, etc.) for persistence."
    fi
    echo ""
    echo "Run 'symphony --help' for usage, or point it at a WORKFLOW.md to start the orchestrator."
}

main "$@"
