#!/usr/bin/env bash
set -euo pipefail

version=v0.10.0
destination=${1:?usage: install-rlsr.sh DESTINATION}
case "$(uname -s)/$(uname -m)" in
    Linux/x86_64)
        platform=linux-x86_64
        checksum=ee774cb6eac5d4545dac0724d550805a01940531db9f7be797f1b57345cda826
        ;;
    Darwin/arm64)
        platform=macos-arm64
        checksum=0859df1112a5f842bc953dcf3bd0a627a2e2714f946498adb8e46ef879b55263
        ;;
    MINGW*_NT-*/x86_64|MSYS*_NT-*/x86_64|CYGWIN_NT-*/x86_64)
        platform=windows-x86_64
        checksum=f585787d903f2c4fcc078080205255b5c8f8945933ead302e80ea0a49a2c341b
        executable=rlsr.exe
        ;;
    *)
        echo 'rlsr setup supports Linux x86_64, macOS arm64, and Windows x86_64 hosts.' >&2
        exit 1
        ;;
esac

if [ "$platform" = windows-x86_64 ] && command -v cygpath >/dev/null 2>&1; then
    destination=$(cygpath -u "$destination")
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
curl --fail --location --silent --show-error \
    "https://github.com/iamd3vil/rlsr/releases/download/$version/rlsr-$version-$platform.zip" \
    -o "$scratch/rlsr.zip"
python_command=python3
if [ "$platform" = windows-x86_64 ]; then python_command=python; fi
"$python_command" - "$scratch/rlsr.zip" "$checksum" <<'PY'
import hashlib
import sys
from pathlib import Path

if hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest() != sys.argv[2]:
    raise SystemExit("rlsr archive checksum mismatch")
PY
if [ "$platform" = windows-x86_64 ]; then
    "$python_command" - "$scratch/rlsr.zip" "$scratch" <<'PY'
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1]) as archive:
    archive.extract("rlsr.exe", sys.argv[2])
PY
else
    unzip -q "$scratch/rlsr.zip" -d "$scratch"
fi
mkdir -p "$destination"
executable=${executable:-rlsr}
cp "$scratch/$executable" "$destination/$executable"
chmod 755 "$destination/$executable"
"$destination/$executable" --version
