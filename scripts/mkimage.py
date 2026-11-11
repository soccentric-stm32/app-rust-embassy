#!/usr/bin/env python3
"""mkimage.py - wrap embassy-app.bin in the BOOT image header.

Author: Sandesh Ghimire <sandesh@soccentric.com>
SRS:    RGEN-001; BOOT BHOST-001, IMG-001..IMG-003

The header is the 256-byte format of SRS-BOOT-F767 section 5.1.  This small
copy exists because the bootloader repository does not yet ship its host
tool; it reads the same Cargo.toml version and git commit as the firmware.
"""

from __future__ import annotations

import argparse
import datetime as dt
import re
import struct
import subprocess
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEADER_SIZE = 256
IMAGE_MAGIC = 0x48574249  # "IBWH" little-endian
HEADER_VERSION = 1
IMAGE_TYPE_APPLICATION = 1
LOAD_ADDRESS = 0x0800C000
SLOT_SIZE = 976 * 1024
RAM_START = 0x20000000
RAM_END = 0x20080000
DEVICE_NAME = "embassy-app"


def package_version() -> str:
    text = (ROOT / "Cargo.toml").read_text()
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not match:
        raise SystemExit("mkimage: no version in Cargo.toml")
    return match.group(1)


def git_commit() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def semver(text: str) -> int:
    match = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", text.strip())
    if not match:
        raise SystemExit(f"mkimage: version must be major.minor.patch, got {text!r}")
    major, minor, patch = (int(part) & 0xFF for part in match.groups())
    return (major << 24) | (minor << 16) | patch


def commit_bytes(text: str) -> bytes:
    # BOOT stores a binary SHA-1.  A full 40-digit hash is decoded; anything
    # shorter (for example the 12-digit hash shown by the firmware) is kept as
    # ASCII, which is what the C boot tool does as well.
    text = text.strip()
    if len(text) >= 40:
        try:
            return bytes.fromhex(text[:40])
        except ValueError:
            pass
    return text[:20].encode("ascii", "replace").ljust(20, b"\0")


def main() -> int:
    parser = argparse.ArgumentParser(description="make a BOOT image from embassy-app.bin")
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--version", default=None)
    parser.add_argument("--commit", default=None)
    parser.add_argument("--name", default=DEVICE_NAME)
    parser.add_argument("--timestamp", type=int, default=None)
    args = parser.parse_args()

    data = args.binary.read_bytes()
    if len(data) < 8:
        raise SystemExit("mkimage: binary is too short for a vector table")
    if len(data) > SLOT_SIZE:
        raise SystemExit("mkimage: image does not fit in the 976 KB slot")

    entry = struct.unpack_from("<I", data, 4)[0]
    sp = struct.unpack_from("<I", data, 0)[0]
    if not (RAM_START < sp <= RAM_END) or (sp & 0x7) != 0:
        raise SystemExit("mkimage: initial stack pointer is not in internal RAM")
    if entry & 1 == 0 or not (LOAD_ADDRESS <= (entry & ~1) < LOAD_ADDRESS + len(data)):
        raise SystemExit("mkimage: reset vector is not inside the slot")

    name = args.name.encode("ascii")
    if len(name) > 32:
        raise SystemExit("mkimage: name is longer than 32 bytes")

    version_text = args.version or package_version()
    commit_text = args.commit if args.commit is not None else git_commit()
    commit = commit_bytes(commit_text)

    timestamp = args.timestamp
    if timestamp is None:
        timestamp = int(dt.datetime.now(dt.timezone.utc).timestamp())

    header = bytearray(HEADER_SIZE)
    struct.pack_into("<IHHIIIIIII", header, 0,
                     IMAGE_MAGIC, HEADER_VERSION, HEADER_SIZE,
                     IMAGE_TYPE_APPLICATION, semver(version_text),
                     len(data), zlib.crc32(data) & 0xFFFFFFFF,
                     LOAD_ADDRESS, entry, timestamp)
    header[36:56] = commit
    header[56:88] = name.ljust(32, b"\0")
    struct.pack_into("<I", header, 252, zlib.crc32(header[:252]) & 0xFFFFFFFF)

    args.output.write_bytes(header + data)
    print(f"image: {args.output} ({len(data)} + {HEADER_SIZE} bytes, "
          f"version {version_text}, commit {commit_text[:12] or 'unknown'})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
