#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="${root}/work"
bootstrap="${AEGIS_BOOTSTRAP_DIR:-/var/tmp/aegis-bootstrap}"
out="${root}/out"

if [[ -n "${AEGIS_PKGS_DIR:-}" ]]; then
  pkgs="${AEGIS_PKGS_DIR}"
elif [[ -d "${root}/../aegis-pkgs/scripts" ]]; then
  pkgs="$(cd "${root}/../aegis-pkgs" && pwd)"
else
  pkgs="${work}/aegis-pkgs"
  if [[ ! -d "${pkgs}/.git" ]]; then
    git clone --depth 1 https://github.com/AegisOperativeSystem/aegis-pkgs.git "${pkgs}"
  fi
fi

export AEGIS_OS_PATH="${root}"
mkdir -p "${bootstrap}" "${out}"
rm -rf "${work}/profile"
mkdir -p "${work}"
cp -a "${root}/profiles/live" "${work}/profile"
sed -i "s|file:///var/tmp/aegis-bootstrap|file://${bootstrap}|g" "${work}/profile/pacman.conf"

if [[ "$(id -u)" -eq 0 ]]; then
  pacman -Sy --noconfirm --needed base-devel sudo rust cargo pkgconf gtk4 gtk4-layer-shell archiso
fi

"${pkgs}/scripts/build-all.sh" "${bootstrap}"
mkarchiso -v -w "${work}/mkarchiso" -o "${out}" "${work}/profile"
echo "ISO written to ${out}"
