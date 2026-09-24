#!/usr/bin/env bash
# Syntropd modeld Uninstaller
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BINDIR="${DESTDIR:-}${PREFIX}/bin"
SYSTEMD_SYSTEM_DIR="${DESTDIR:-}/etc/systemd/system"
SYSUSERS_DIR="${DESTDIR:-}/usr/lib/sysusers.d"
TMPFILES_DIR="${DESTDIR:-}/usr/lib/tmpfiles.d"

echo "=== Uninstalling Syntropd modeld ==="

if [ "$(id -u)" -eq 0 ]; then
    systemctl stop modeld.service modeld.socket || true
    systemctl disable modeld.service modeld.socket || true
fi

rm -f "${BINDIR}/modeld" "${BINDIR}/modelctl"
rm -f "${SYSTEMD_SYSTEM_DIR}/modeld.service" "${SYSTEMD_SYSTEM_DIR}/modeld.socket"
rm -f "${SYSUSERS_DIR}/modeld.conf" "${TMPFILES_DIR}/modeld.conf"

if [ "$(id -u)" -eq 0 ]; then
    systemctl daemon-reload || true
fi

echo "modeld uninstalled successfully."
