#!/bin/sh
set -eu

REPO="trk/commiter"
BINARY="commiter"
INSTALL_DIR="${HOME}/.local/bin"

OS="$(uname -s)"
ARCH="$(uname -m)"

if [ "${OS}" != "Linux" ]; then
	echo "Unsupported OS: ${OS}" >&2
	exit 1
fi

if [ "${ARCH}" != "x86_64" ]; then
	echo "Unsupported architecture: ${ARCH}" >&2
	exit 1
fi

mkdir -p "${INSTALL_DIR}"

echo "Downloading ${BINARY} from ${REPO}..."
curl -fsSL "https://github.com/${REPO}/releases/latest/download/${BINARY}" \
	-o "${INSTALL_DIR}/${BINARY}"
chmod +x "${INSTALL_DIR}/${BINARY}"

echo "✓ Installed to ${INSTALL_DIR}/${BINARY}"

case ":${PATH}:" in
	*:"${INSTALL_DIR}":*) ;;
	*)
		echo
		echo "⚠  ${INSTALL_DIR} is not in your PATH."
		echo "   Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):"
		echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
		;;
esac
