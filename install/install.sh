#!/usr/bin/env bash
# Syntropd modeld Idempotent Installer
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BINDIR="${DESTDIR:-}${PREFIX}/bin"
SYSTEMD_SYSTEM_DIR="${DESTDIR:-}/etc/systemd/system"
SYSUSERS_DIR="${DESTDIR:-}/usr/lib/sysusers.d"
TMPFILES_DIR="${DESTDIR:-}/usr/lib/tmpfiles.d"

echo "=== Installing Syntropd modeld ==="

# 1. Compile release binaries
echo "[1/5] Building release artifacts via Cargo..."
cargo build --release --workspace

# 2. Deploy executables
echo "[2/5] Installing binaries into ${BINDIR}..."
install -d "${BINDIR}"
install -m 0755 target/release/modeld "${BINDIR}/modeld"
install -m 0755 target/release/modelctl "${BINDIR}/modelctl"

# 3. Deploy systemd packaging
echo "[3/5] Installing system configuration files..."
install -d "${SYSTEMD_SYSTEM_DIR}" "${SYSUSERS_DIR}" "${TMPFILES_DIR}"
install -m 0644 systemd/modeld.service "${SYSTEMD_SYSTEM_DIR}/modeld.service"
install -m 0644 systemd/modeld.socket "${SYSTEMD_SYSTEM_DIR}/modeld.socket"
install -m 0644 sysusers.d/modeld.conf "${SYSUSERS_DIR}/modeld.conf"
install -m 0644 tmpfiles.d/modeld.conf "${TMPFILES_DIR}/modeld.conf"

# 4. Provision system user and directories if running as root
if [ "$(id -u)" -eq 0 ]; then
    echo "[4/5] Provisioning system users and tmpfiles..."
    systemd-sysusers || true
    systemd-tmpfiles --create || true
    systemctl daemon-reload || true
    echo "[5/5] modeld installed successfully."
    echo "To activate: sudo systemctl enable --now modeld.socket"
else
    echo "[4/5] Non-root install completed without systemctl reload."
    echo "[5/5] Binaries deployed to ${BINDIR}."
fi
