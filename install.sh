#!/usr/bin/env bash
# sshr installer — downloads a prebuilt binary from GitHub releases.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/hoangneeee/sshr/master/install.sh | bash
#
# Environment overrides:
#   VERSION       Release tag to install (default: latest). Examples: v0.10.4, stable
#   INSTALL_DIR   Where to put the sshr binary (default: /usr/local/bin)
#   REPO          GitHub repo, owner/name (default: hoangneeee/sshr)

set -euo pipefail

REPO="${REPO:-hoangneeee/sshr}"
VERSION="${VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
BIN_NAME="sshr"

red()    { printf '\033[31m%s\033[0m\n' "$*" >&2; }
green()  { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }
info()   { printf '  %s\n' "$*"; }

die() { red "error: $*"; exit 1; }

require() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

require curl
require tar
require uname
require mktemp

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)  os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    *) die "unsupported OS: $os (sshr ships binaries for Linux and macOS only)" ;;
  esac

  case "$arch" in
    x86_64|amd64) arch_part="x86_64" ;;
    arm64|aarch64)
      if [ "$os" = "Darwin" ]; then
        arch_part="aarch64"
      else
        die "no prebuilt sshr binary for linux/$arch yet — build from source: https://github.com/$REPO"
      fi
      ;;
    *) die "unsupported architecture: $arch" ;;
  esac

  echo "${arch_part}-${os_part}"
}

main() {
  local target asset url tmpdir archive
  target="$(detect_target)"
  asset="${BIN_NAME}-${target}.tar.gz"
  url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"

  yellow "Installing ${BIN_NAME} (${VERSION}) for ${target}"
  info "from: ${url}"
  info "to:   ${INSTALL_DIR}/${BIN_NAME}"

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT
  archive="${tmpdir}/${asset}"

  if ! curl -fL --progress-bar -o "$archive" "$url"; then
    die "download failed. Check that release '${VERSION}' exists at https://github.com/${REPO}/releases"
  fi

  tar -xzf "$archive" -C "$tmpdir"
  [ -f "${tmpdir}/${BIN_NAME}" ] || die "archive did not contain expected '${BIN_NAME}' binary"
  chmod +x "${tmpdir}/${BIN_NAME}"

  local sudo=""
  if [ ! -w "$INSTALL_DIR" ]; then
    if command -v sudo >/dev/null 2>&1; then
      sudo="sudo"
      yellow "${INSTALL_DIR} is not writable — using sudo"
    else
      die "${INSTALL_DIR} is not writable and sudo is unavailable. Re-run with INSTALL_DIR=\$HOME/.local/bin"
    fi
  fi

  $sudo mkdir -p "$INSTALL_DIR"
  $sudo install -m 0755 "${tmpdir}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"

  green "✓ Installed ${BIN_NAME} to ${INSTALL_DIR}/${BIN_NAME}"

  case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *) yellow "note: ${INSTALL_DIR} is not in your PATH — add it to your shell profile" ;;
  esac

  if command -v "$BIN_NAME" >/dev/null 2>&1; then
    info "$("$BIN_NAME" --version 2>/dev/null || echo "${BIN_NAME} installed")"
  fi
}

main "$@"
