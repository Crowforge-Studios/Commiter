#!/bin/sh
set -eu

# ========== config ==========
GITHUB="Crowforge-Studios/Commiter"
BINARY="commiter"
API_URL="https://api.github.com/repos/${GITHUB}/releases/latest"
DOWNLOAD_URL="https://github.com/${GITHUB}/releases/latest/download/${BINARY}"

# ========== colors (tput) ==========
if command -v tput >/dev/null 2>&1 && [ -t 1 ]; then
	RED=$(tput setaf 1)
	GREEN=$(tput setaf 2)
	YELLOW=$(tput setaf 3)
	CYAN=$(tput setaf 6)
	BOLD=$(tput bold)
	NC=$(tput sgr0)
else
	RED=""; GREEN=""; YELLOW=""; CYAN=""; BOLD=""; NC=""
fi

info()  { printf "${GREEN}✓${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}⚠${NC} %s\n" "$*" >&2; }
err()   { printf "${RED}✗${NC} %s\n" "$*" >&2; }
die()   { err "$1"; exit 1; }

# ========== flags ==========
INSTALL_DIR="${HOME}/.local/bin"
DO_UNINSTALL=0
DO_VERSION=""
SHOW_HELP=0

usage() {
	cat <<EOF
Usage: install.sh [OPTIONS]

Install ${BINARY} from GitHub releases.

Options:
  -p, --prefix DIR   Install to DIR (default: ~/.local/bin)
  -u, --uninstall    Remove installed binary
  -v, --version TAG  Install specific version (default: latest)
  -h, --help         Show this help
EOF
	exit 0
}

while [ $# -gt 0 ]; do
	case "$1" in
		-p|--prefix)   INSTALL_DIR="$2"; shift 2 ;;
		-u|--uninstall) DO_UNINSTALL=1; shift ;;
		-v|--version)  DO_VERSION="$2"; shift 2 ;;
		-h|--help)     SHOW_HELP=1; shift ;;
		*)             die "Unknown option: $1. Use -h for help." ;;
	esac
done

[ "$SHOW_HELP" = 1 ] && usage

# ========== uninstall ==========
if [ "$DO_UNINSTALL" = 1 ]; then
	TARGET="${INSTALL_DIR}/${BINARY}"
	if [ -f "$TARGET" ]; then
		rm -f "$TARGET"
		info "Removed ${TARGET}"
	else
		warn "Not found: ${TARGET}"
	fi
	exit 0
fi

# ========== prereqs ==========
command -v curl >/dev/null 2>&1 || die "curl is required but not installed."
command -v uname >/dev/null 2>&1 || die "uname is required."
command -v stat >/dev/null 2>&1 || die "stat is required."

# ========== platform ==========
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
	Linux)   ;;
	Darwin)  die "macOS is not supported yet." ;;
	*)       die "Unsupported OS: ${OS}" ;;
esac

case "${ARCH}" in
	x86_64|amd64)  ;;
	aarch64|arm64) die "ARM64 is not supported yet." ;;
	*)             die "Unsupported architecture: ${ARCH}" ;;
esac

# ========== version resolution ==========
if [ -n "$DO_VERSION" ]; then
	VERSION_TAG="${DO_VERSION}"
	ASSET_URL="https://github.com/${GITHUB}/releases/download/${VERSION_TAG}/${BINARY}"
	info "Installing version ${VERSION_TAG}"
else
	info "Fetching latest release info..."
	VERSION_TAG="$(curl -sfL "${API_URL}" 2>/dev/null | \
		grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' | \
		cut -d'"' -f4)"
	if [ -z "$VERSION_TAG" ]; then
		warn "Could not determine latest version, falling back to 'latest'"
		VERSION_TAG="latest"
		ASSET_URL="${DOWNLOAD_URL}"
	else
		ASSET_URL="https://github.com/${GITHUB}/releases/download/${VERSION_TAG}/${BINARY}"
		info "Latest version: ${CYAN}${VERSION_TAG}${NC}"
	fi
fi

# ========== prepare ==========
mkdir -p "${INSTALL_DIR}"
TMP_BIN="$(mktemp -t "${BINARY}.XXXXXXXXXX")"
trap 'rm -f "$TMP_BIN"' EXIT INT TERM

# ========== backup existing ==========
OLD_BIN="${INSTALL_DIR}/${BINARY}"
HAD_OLD=0
[ -f "$OLD_BIN" ] && HAD_OLD=1

if [ "$HAD_OLD" = 1 ]; then
	# Try to verify old binary is actually from us (it's the same name)
	info "Backing up current binary..."
	cp "$OLD_BIN" "${OLD_BIN}.bak" 2>/dev/null || true
fi

# ========== download ==========
printf "Downloading ${BINARY}..."
set +e
CURL_OUTPUT="$(curl -#fSL -o "$TMP_BIN" -w '%{http_code}' "$ASSET_URL" 2>&1)"
CURL_EXIT=$?
set -e

HTTP_CODE="$(printf '%s' "$CURL_OUTPUT" | tail -c 3)"

if [ "$CURL_EXIT" -ne 0 ] || [ "$HTTP_CODE" != "200" ]; then
	rm -f "$TMP_BIN"
	[ "$HAD_OLD" = 1 ] && mv "${OLD_BIN}.bak" "$OLD_BIN" 2>/dev/null || true
	die "Download failed (HTTP ${HTTP_CODE})"
fi

# ========== validate ==========
# Must be an ELF binary
if ! file "$TMP_BIN" 2>/dev/null | grep -qiE 'ELF|executable' >/dev/null 2>&1; then
	rm -f "$TMP_BIN"
	[ "$HAD_OLD" = 1 ] && mv "${OLD_BIN}.bak" "$OLD_BIN" 2>/dev/null || true
	die "Downloaded file is not a valid executable (not ELF). Corrupt release?"
fi

# Must be at least 500KB (reasonable minimum for a Rust binary)
MIN_SIZE=$((500 * 1024))
ACTUAL_SIZE="$(stat -c%s "$TMP_BIN" 2>/dev/null || echo 0)"
if [ "$ACTUAL_SIZE" -lt "$MIN_SIZE" ]; then
	rm -f "$TMP_BIN"
	[ "$HAD_OLD" = 1 ] && mv "${OLD_BIN}.bak" "$OLD_BIN" 2>/dev/null || true
	die "Downloaded file too small (${ACTUAL_SIZE} bytes). Corrupt download?"
fi

# ========== install ==========
chmod +x "$TMP_BIN"
mv "$TMP_BIN" "$OLD_BIN"

# Clean up backup
rm -f "${OLD_BIN}.bak"

SIZE_KB=$(( ACTUAL_SIZE / 1024 ))
info "Installed ${BOLD}${OLD_BIN}${NC} (${SIZE_KB}K)"

# ========== PATH check ==========
case ":${PATH}:" in
	*:"${INSTALL_DIR}":*) ;;
	*)
		echo ""
		warn "${INSTALL_DIR} is not in your PATH."
		echo ""
		echo "   Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):"
		echo ""
		printf "   ${CYAN}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}\n"
		echo ""
		;;
esac

# ========== next steps ==========
echo ""
info "Run ${CYAN}${BINARY}${NC} inside any git repository to start."
info "Press ${CYAN}s${NC} for Settings (update / uninstall)."
echo ""
