#!/usr/bin/env sh
# Install rust-symphony CLI from GitHub Releases.
# Usage: curl -fsSL https://raw.githubusercontent.com/Industrial/rust-symphony/main/install.sh | sh
# Or:    curl -fsSL https://rust-symphony.sh/install | sh  (when domain is configured)
#
# Optional: RUST_SYMPHONY_VERSION=v1.2.3 to pin version (default: latest release)
# Optional: RUST_SYMPHONY_INSTALL_DIR=/path to set install directory (default: ~/.local/bin)

set -e

REPO="Industrial/rust-symphony"
RELEASES_URL="https://github.com/${REPO}/releases"
API_URL="https://api.github.com/repos/${REPO}/releases"

# Resolve install directory
get_install_dir() {
    if [ -n "${RUST_SYMPHONY_INSTALL_DIR}" ]; then
        echo "${RUST_SYMPHONY_INSTALL_DIR}"
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
    echo "See ${RELEASES_URL} for manual download, or install with: cargo install symphony-agent"
    exit 1
}

# Get latest release tag (without 'v' prefix for URL) or use RUST_SYMPHONY_VERSION
get_version() {
    if [ -n "${RUST_SYMPHONY_VERSION}" ]; then
        # Strip leading 'v' if present
        echo "${FORGE_VERSION}" | sed 's/^v//'
        return
    fi
    # Fetch latest tag from GitHub API (no jq required); strip leading 'v'
    RAW=$(curl -sSf "${API_URL}/latest" | grep '"tag_name":' | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/' | head -1)
    TAG=$(echo "${RAW}" | sed 's/^v//')
    if [ -z "${TAG}" ]; then
        echo "Could not determine latest release. Set RUST_SYMPHONY_VERSION or check ${RELEASES_URL}"
        exit 1
    fi
    echo "${TAG}"
}

main() {
    echo "Installing rust-symphony CLI..."
    detect_platform
    VERSION=$(get_version)
    INSTALL_DIR=$(get_install_dir)
    # Release assets use tag with 'v' in the URL
    TAG_V="v${VERSION}"
    ARCHIVE="rust-symphony-${TAG_V}-${TARGET}.${EXT}"
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

    # Binary is named 'rust-symphony' (or rust-symphony.exe on Windows)
    if [ -f "rust-symphony" ]; then
        chmod +x rust-symphony
        mv rust-symphony "${INSTALL_DIR}/rust-symphony"
    else
        echo "Archive did not contain expected 'rust-symphony' binary."
        exit 1
    fi

    echo "Installed rust-symphony to ${INSTALL_DIR}/rust-symphony"
    if ! echo "${PATH}" | grep -q "${INSTALL_DIR}"; then
        echo ""
        echo "Add rust-symphony to your PATH:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo "Add the above to your shell profile (~/.bashrc, ~/.zshrc, etc.) for persistence."
    fi
    echo ""
    echo "Run 'rust-symphony new myapp' to create an app, then 'rust-symphony dev' to start."
}

main "$@"
