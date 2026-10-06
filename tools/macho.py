"""Minimal Mach-O / FAT inspector: architectures, dylibs, rpaths, build version, strings."""
from __future__ import annotations

import re
import struct
import sys
from pathlib import Path

MH_MAGIC = 0xFEEDFACE
MH_CIGAM = 0xCEFAEDFE
MH_MAGIC_64 = 0xFEEDFACF
MH_CIGAM_64 = 0xCFFAEDFE
FAT_MAGIC = 0xCAFEBABE
FAT_CIGAM = 0xBEBAFECA
FAT_MAGIC_64 = 0xCAFEBABF
FAT_CIGAM_64 = 0xBFBAFECA

CPU_TYPES = {7: "x86", 12: "arm", 16777223: "x86_64", 16777228: "arm64"}
CPU_SUB = {"arm64": {0: "all", 1: "v8", 2: "e"}, "x86_64": {3: "all", 8: "h"}}

LC = {
    0x0C: "LC_LOAD_DYLIB", 0x18: "LC_LOAD_WEAK_DYLIB", 0x1F: "LC_REEXPORT_DYLIB",
    0x23: "LC_LOAD_UPWARD_DYLIB", 0x0D: "LC_ID_DYLIB", 0x8000001C: "LC_LOAD_DYLIB(lazy)",
    0x80000018: "LC_LOAD_WEAK_DYLIB(lazy)", 0x8000001F: "LC_REEXPORT_DYLIB(lazy)",
    0x0E: "LC_LOAD_DYLINKER", 0x1B: "LC_UUID", 0x32: "LC_BUILD_VERSION",
    0x24: "LC_VERSION_MIN_MACOSX", 0x25: "LC_VERSION_MIN_IPHONEOS",
    0x28: "LC_MAIN", 0x1D: "LC_CODE_SIGNATURE", 0x22: "LC_DYLD_INFO_ONLY",
    0x80000022: "LC_DYLD_INFO", 0x21: "LC_ENCRYPTION_INFO", 0x2C: "LC_ENCRYPTION_INFO_64",
    0x26: "LC_FUNCTION_STARTS", 0x2B: "LC_DATA_IN_CODE", 0x29: "LC_DATA_IN_CODE",
    0x0F: "LC_LOAD_DYLINKER", 0x1C: "LC_RPATH", 0x2A: "LC_SOURCE_VERSION",
}

PLATFORMS = {1: "macOS", 2: "iOS", 3: "tvOS", 4: "watchOS", 5: "bridgeOS",
             6: "macCatalyst", 7: "iOSSimulator", 8: "tvOSSimulator",
             9: "watchOSSimulator", 10: "driverKit", 11: "visionOS", 12: "visionOSSimulator"}

TOOLS = {1: "clang", 2: "swift", 3: "ld", 4: "lld", 5: "metal", 6: "airnt",
         7: "air-lld", 8: "cctools"}


def _read_lc_str(buf: bytes, off: int, cmd_off: int) -> str:
    end = buf.index(b"\x00", off)
    return buf[off:end].decode("utf-8", "replace")


def parse_macho(buf: bytes, base: int = 0) -> dict:
    magic, = struct.unpack_from(">I", buf, base)
    if magic in (MH_CIGAM_64, MH_CIGAM):
        little, is64 = True, magic == MH_CIGAM_64
    elif magic in (MH_MAGIC_64, MH_MAGIC):
        little, is64 = False, magic == MH_MAGIC_64
    else:
        return {"error": f"bad magic {magic:#x}"}
    endian = "<" if little else ">"
    cputype, cpusub, _ft, ncmds, _sz, flags, _rsv = struct.unpack_from(endian + "IiiIIII", buf, base + 4)

    cpu = CPU_TYPES.get(cputype, f"cpu{cputype}")
    out = {
        "cpu": cpu,
        "subtype": CPU_SUB.get(cpu, {}).get(cpusub & 0xFFFFFF, str(cpusub)),
        "64bit": is64,
        "ncmds": ncmds,
        "flags": flags,
        "dylibs": [],
        "rpaths": [],
        "build": [],
        "min": [],
        "encrypted": None,
    }

    off = base + (32 if is64 else 28)
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack_from(endian + "II", buf, off)
        name = LC.get(cmd, f"LC_{cmd:#x}")
        if cmd in (0x0C, 0x18, 0x1F, 0x23, 0x0D, 0x8000001C, 0x80000018, 0x8000001F) and cmdsize > 24:
            n, = struct.unpack_from(endian + "I", buf, off + 8)
            out["dylibs"].append(_read_lc_str(buf, off + n, off))
        elif cmd == 0x1C:
            n, = struct.unpack_from(endian + "I", buf, off + 8)
            out["rpaths"].append(_read_lc_str(buf, off + n, off))
        elif cmd == 0x32:
            plat, minos, sdk, nt = struct.unpack_from(endian + "IIII", buf, off + 8)
            tools = []
            for i in range(nt):
                t, v = struct.unpack_from(endian + "II", buf, off + 24 + i * 8)
                tools.append(f"{TOOLS.get(t, t)}={v >> 16}.{v >> 8 & 0xFF}.{v & 0xFF}")
            out["build"].append(
                f"{PLATFORMS.get(plat, plat)} min={minos >> 16}.{minos >> 8 & 0xFF}.{minos & 0xFF} "
                f"sdk={sdk >> 16}.{sdk >> 8 & 0xFF}.{sdk & 0xFF} tools=[{', '.join(tools)}]")
        elif cmd in (0x24, 0x25):
            v, s = struct.unpack_from(endian + "II", buf, off + 8)
            out["min"].append(f"{name} {v >> 16}.{v >> 8 & 0xFF}.{v & 0xFF}")
        elif cmd in (0x21, 0x2C):
            cryptoff, cryptsize, cryptid = struct.unpack_from(endian + "III", buf, off + 8)
            out["encrypted"] = (cryptid, cryptoff, cryptsize)
        elif name == "LC_UUID":
            out["uuid"] = buf[off + 8:off + 24].hex()
        off += cmdsize
    return out


ASCII = re.compile(rb"[\x20-\x7e]{4,}")


def strings(buf: bytes, minlen: int = 5) -> list[str]:
    pat = re.compile(rb"[\x20-\x7e]{%d,}" % minlen)
    return [m.group().decode("ascii", "replace") for m in pat.finditer(buf)]


def main() -> None:
    for arg in sys.argv[1:]:
        p = Path(arg)
        if not p.is_file():
            print(f"!! missing {p}")
            continue
        buf = p.read_bytes()
        print(f"\n=== {p.name}  ({len(buf):,} bytes) ===")
        magic, = struct.unpack_from(">I", buf, 0)
        if magic in (FAT_MAGIC, FAT_MAGIC_64, FAT_CIGAM, FAT_CIGAM_64):
            little = magic in (FAT_CIGAM, FAT_CIGAM_64)
            endian = ">" if not little else "<"
            if little:
                endian = "<"
            nfat, = struct.unpack_from(">I", buf, 4)
            print(f"  FAT universal, {nfat} slices")
            off = 8
            for _ in range(nfat):
                cput, cpus, o, sz, align = struct.unpack_from(">iiIII", buf, off)
                cpu = CPU_TYPES.get(cput, cput)
                print(f"  - slice {cpu}: offset={o} size={sz} align=2^{align}")
                info = parse_macho(buf, o)
                print(f"      sub={info['subtype']} 64bit={info['64bit']}")
                for b in info["build"]:
                    print(f"      build: {b}")
                if info.get("rpaths"):
                    print(f"      rpaths: {info['rpaths']}")
                print("      dylibs:")
                for d in info["dylibs"]:
                    print(f"        {d}")
                off += 20
        else:
            info = parse_macho(buf)
            print(f"  cpu={info['cpu']} sub={info['subtype']} 64bit={info['64bit']}")
            for b in info.get("build", []):
                print(f"  build: {b}")
            print(f"  uuid={info.get('uuid')}  encrypted={info.get('encrypted')}")
            print(f"  rpaths: {info['rpaths']}")
            print("  dylibs:")
            for d in info["dylibs"]:
                print(f"    {d}")


if __name__ == "__main__":
    main()
