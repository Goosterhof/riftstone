"""Read a Dragon's Dogma 2 save for the privacy sweep: the private strings it holds.

A save is a DSSS container, encrypted with the cipher the community calls Mandarin (the math
after MandarinJuice, MIT, © 2026 Michał Gębicki) under the Steam account id, then deflated,
then a self-describing stream of classes (BUILD-SPEC § v0.4 has the format). The sweep needs
only the strings a person typed or another player chose: the Arisen's and pawns' names and
nicknames, the hired pawns' creators' names, and the pawns' Rift search ids. It reads; it never
writes a save.

The privacy sweep loads this file by path (it runs isolated, `python3 -I`). AES is pure Python
here, so the sweep's self-test needs no package; `cryptography` speeds a real run when present.
"""

from __future__ import annotations

import struct
import zlib

M64 = (1 << 64) - 1
P = int.from_bytes(bytes.fromhex("f33b6fb972a0b72515e45c391829e182ad8a9bdc0a64d3444d79c810ab863717"), "little")
Q = int.from_bytes(bytes.fromhex("f99db75c39d0db920a72ae1c8c9470c156c54d6e05b269a2a63c648855c39b0b"), "little")
G = int.from_bytes(bytes.fromhex("e66f544afcce68c5ef07b9a07b277585344a1db61376e831f73b9fbd5f44f715"), "little")
HEADER_KEY = pow(G, 20 % Q, P)
SEED = 0x5EC646997D69AE1B
SLICE_HEADER = 528
FOOTER = 0x8C
STEAM_ID64_BASE = 76561197960265728

# The fields whose strings are a person's, by MurmurHash3 x86_32 (seed 0xFFFFFFFF) of the name.
PRIVATE_FIELDS = ("Name", "NickName", "_Name", "_Nickname", "_NickName", "_SearchId")
# Two record types whose `Name` is the game's own id, never a person's: state names that carry a
# number, object ids (`xx000000_…_0000000000`) and GUIDs. A played save holds thousands of them,
# every one with a digit in it, and each would cost the sweep a pattern for nothing (2,767 on the
# bench's eight saves, 2026-10-18). Their type names are not known here, so they go by type hash.
GAME_ID_RECORDS = frozenset({0xA5E674E7, 0x7E5E2FB9})


class SaveError(Exception):
    pass


# --- AES-128, the forward cipher (OFB needs nothing else) ------------------------------------

SBOX = bytes.fromhex(
    "637c777bf26b6fc53001672bfed7ab76ca82c97dfa5947f0add4a2af9ca472c0"
    "b7fd9326363ff7cc34a5e5f171d8311504c723c31896059a071280e2eb27b275"
    "09832c1a1b6e5aa0523bd6b329e32f8453d100ed20fcb15b6acbbe394a4c58cf"
    "d0efaafb434d338545f9027f503c9fa851a3408f929d38f5bcb6da2110fff3d2"
    "cd0c13ec5f974417c4a77e3d645d197360814fdc222a908846eeb814de5e0bdb"
    "e0323a0a4906245cc2d3ac629195e479e7c8376d8dd54ea96c56f4ea657aae08"
    "ba78252e1ca6b4c6e8dd741f4bbd8b8a703eb5664803f60e613557b986c11d9e"
    "e1f8981169d98e949b1e87e9ce5528df8ca1890dbfe6426841992d0fb054bb16"
)
RCON = (0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1B, 0x36)


def _xtime(b: int) -> int:
    b <<= 1
    return (b ^ 0x11B) if b & 0x100 else b


def _expand(key: bytes) -> list[list[int]]:
    words = [list(key[i : i + 4]) for i in range(0, 16, 4)]
    for i in range(4, 44):
        t = list(words[i - 1])
        if i % 4 == 0:
            t = t[1:] + t[:1]
            t = [SBOX[b] for b in t]
            t[0] ^= RCON[i // 4 - 1]
        words.append([a ^ b for a, b in zip(words[i - 4], t)])
    return [sum(words[r * 4 : r * 4 + 4], []) for r in range(11)]


def _encrypt_block(round_keys: list[list[int]], block: bytes) -> bytes:
    s = [b ^ k for b, k in zip(block, round_keys[0])]
    for r in range(1, 11):
        s = [SBOX[b] for b in s]
        s = [s[(i + 4 * (i % 4)) % 16] for i in range(16)]  # ShiftRows (column-major state)
        if r != 10:
            mixed = []
            for c in range(4):
                a = s[c * 4 : c * 4 + 4]
                total = a[0] ^ a[1] ^ a[2] ^ a[3]
                mixed += [a[i] ^ total ^ _xtime(a[i] ^ a[(i + 1) % 4]) for i in range(4)]
            s = mixed
        s = [b ^ k for b, k in zip(s, round_keys[r])]
    return bytes(s)


def _ofb_pure(key: bytes, iv: bytes, data: bytes) -> bytes:
    round_keys = _expand(key)
    out = bytearray(len(data))
    block = iv
    for at in range(0, len(data), 16):
        block = _encrypt_block(round_keys, block)
        chunk = data[at : at + 16]
        out[at : at + len(chunk)] = bytes(a ^ b for a, b in zip(chunk, block))
    return bytes(out)


def ofb(key: bytes, iv: bytes, data: bytes) -> bytes:
    try:
        from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
    except ImportError:
        return _ofb_pure(key, iv, data)
    return Cipher(algorithms.AES(key), modes.OFB(iv)).encryptor().update(data)


# --- Mandarin -------------------------------------------------------------------------------


def _splitmix(state: int) -> int:
    state = (state + 0x9E3779B97F4A7C15) & M64
    state = ((state ^ (state >> 30)) * 0xBF58476D1CE4E5B9) & M64
    state = ((state ^ (state >> 27)) * 0x94D049BB133111EB) & M64
    return state ^ (state >> 31)


def _slice_units(length: int) -> tuple[list[int], int]:
    state, units = SEED, []
    for _ in range((length >> 14) + 1):
        if length > 0:
            unit = (state & 7) + 1
            units.append(unit)
            length -= min(length, unit << 14)
        state = _splitmix(state)
    return units, state


def _header_stream(state: int) -> tuple[bytes, int]:
    for _ in range(16):
        state = _splitmix(state)
    out = bytearray(SLICE_HEADER)
    for i in range(SLICE_HEADER):
        state = _splitmix(state)
        out[i] = state & 0xFF
    return bytes(out), state


def murmur3(data: bytes, seed: int = 0xFFFFFFFF) -> int:
    c1, c2, h, n = 0xCC9E2D51, 0x1B873593, seed, len(data)
    for i in range(0, n - n % 4, 4):
        k = int.from_bytes(data[i : i + 4], "little")
        k = (k * c1) & 0xFFFFFFFF
        k = ((k << 15) | (k >> 17)) & 0xFFFFFFFF
        h ^= (k * c2) & 0xFFFFFFFF
        h = ((h << 13) | (h >> 19)) & 0xFFFFFFFF
        h = (h * 5 + 0xE6546B64) & 0xFFFFFFFF
    k = 0
    for i, b in enumerate(data[n - n % 4 :]):
        k |= b << (8 * i)
    if n % 4:
        k = (k * c1) & 0xFFFFFFFF
        k = ((k << 15) | (k >> 17)) & 0xFFFFFFFF
        h ^= (k * c2) & 0xFFFFFFFF
    h ^= n
    h ^= h >> 16
    h = (h * 0x85EBCA6B) & 0xFFFFFFFF
    h ^= h >> 13
    h = (h * 0xC2B2AE35) & 0xFFFFFFFF
    return h ^ (h >> 16)


def open_save(raw: bytes, account_id: int) -> bytes:
    """The payload of a save (decrypted, inflated). SaveError when it does not open."""
    if raw[:4] != b"DSSS" or len(raw) < 16 + FOOTER or struct.unpack_from("<I", raw, 4)[0] != 2:
        raise SaveError("not a DSSS v2 save")
    flags = struct.unpack_from("<I", raw, 8)[0]
    length = struct.unpack_from("<Q", raw, len(raw) - 12)[0]
    data = raw[16 : len(raw) - FOOTER]
    units, state = _slice_units(length)
    if sum((u << 14) + SLICE_HEADER for u in units) != len(data):
        raise SaveError("slice layout")
    uid = ~account_id & M64
    state = (state + uid) & M64
    secret = pow(HEADER_KEY, uid, P)
    out, pos, remaining = bytearray(), 0, length
    for unit in units:
        size = unit << 14
        stream, state = _header_stream(state)
        header = bytes(a ^ b for a, b in zip(data[pos : pos + SLICE_HEADER], stream))
        key = b""
        for k in range(4):
            if int.from_bytes(header[k * 128 : k * 128 + 64], "little") != HEADER_KEY:
                raise SaveError("another account's save")
            part = int.from_bytes(header[k * 128 + 64 : k * 128 + 128], "little") // secret
            key += (part & M64).to_bytes(8, "little")
        body = ofb(key[:16], key[16:], data[pos + SLICE_HEADER : pos + SLICE_HEADER + size])
        out += body[: min(size, remaining)]
        remaining -= min(size, remaining)
        pos += SLICE_HEADER + size
    if flags & 0x08:
        zsize = struct.unpack_from("<I", out, 12)[0]
        return zlib.decompress(bytes(out[24 : 24 + zsize]), -15)
    return bytes(out)


def seal(payload: bytes, account_id: int) -> bytes:
    """Encrypt an invented payload into a save — the self-test's only use."""
    units, state = _slice_units(len(payload))
    uid = ~account_id & M64
    state = (state + uid) & M64
    personal = pow(pow(G, uid % Q, P), 20, P)
    out = bytearray(b"DSSS" + struct.pack("<III", 2, 0x10, 0))
    done = 0
    for unit in units:
        size = unit << 14
        probe, key = state, bytearray(32)
        for i in range(16):
            probe = _splitmix(probe)
            key[i], key[16 + i] = probe & 0xFF, (probe >> 8) & 0xFF
        header = bytearray(SLICE_HEADER)
        for k in range(4):
            header[k * 128 : k * 128 + 64] = HEADER_KEY.to_bytes(64, "little")
            part = int.from_bytes(key[k * 8 : k * 8 + 8], "little")
            header[k * 128 + 64 : k * 128 + 128] = (personal * part).to_bytes(64, "little")
        stream, state = _header_stream(state)
        body = bytearray(size)
        take = min(size, len(payload) - done)
        body[:take] = payload[done : done + take]
        out += bytes(a ^ b for a, b in zip(header, stream))
        out += ofb(bytes(key[:16]), bytes(key[16:]), bytes(body))
        done += take
    out += bytes(128) + struct.pack("<Q", len(payload))
    out += struct.pack("<I", murmur3(bytes(out)))
    return bytes(out)


# --- the class stream: only its strings ------------------------------------------------------


def private_strings(payload: bytes) -> list[str]:
    """Every string a PRIVATE_FIELDS field holds anywhere in the stream, outside the game's
    own id records (GAME_ID_RECORDS)."""
    wanted = {murmur3(name.encode()) for name in PRIVATE_FIELDS}
    buf, found = bytes(16) + payload, []
    pos = 16

    def align(n: int) -> None:
        nonlocal pos
        if n > 1:
            pos = (pos + n - 1) & ~(n - 1)

    def u32() -> int:
        nonlocal pos
        v = struct.unpack_from("<I", buf, pos)[0]
        pos += 4
        return v

    def i32() -> int:
        nonlocal pos
        v = struct.unpack_from("<i", buf, pos)[0]
        pos += 4
        return v

    def string() -> str:
        nonlocal pos
        align(4)
        n = u32()
        s = buf[pos : pos + 2 * n].decode("utf-16-le", "replace")
        pos += 2 * n
        return s

    def scalar(size: int) -> None:
        nonlocal pos
        align(size)
        pos += size

    def value(kind: int, field: int, depth: int, record: int) -> None:
        if depth > 96:
            raise SaveError("too deep")
        if kind == -1:
            array(depth)
        elif kind == 17:
            klass(depth + 1)
        elif kind == 15:
            s = string()
            if field in wanted and record not in GAME_ID_RECORDS and s.strip():
                found.append(s)
        elif 1 <= kind <= 16:
            align(4)
            scalar(u32())
        else:
            raise SaveError("unknown kind")

    def array(depth: int) -> None:
        nonlocal pos
        align(4)
        member, size, count, kind = i32(), u32(), u32(), i32()
        if count > len(buf):
            raise SaveError("bad count")
        if kind == 1 and struct.unpack_from("<I", buf, pos)[0] == 0xFFEEFFEE:
            pos += 4 + 4 * count
        for _ in range(count):
            if kind == 1:
                klass(depth + 1)
            elif member == 15:
                string()
            else:
                scalar(size)
        align(4)

    def klass(depth: int) -> None:
        count, record = u32(), u32()
        if count > len(buf) // 8:
            raise SaveError("bad field count")
        for _ in range(count):
            field, kind = u32(), i32()
            value(kind, field, depth, record)
            align(4)

    try:
        while pos + 8 <= len(buf) - 7:
            u32()
            klass(0)
    except (struct.error, IndexError) as err:
        raise SaveError("stream ends early") from err
    return found


def write_stream(classes: list[list[tuple[str, str]]], records: list[int] | None = None) -> bytes:
    """An invented stream of classes with string fields — the self-test's only use. `records`
    gives each class's type hash (`app.Invented` when left out)."""
    out = bytearray()

    def pad(n: int) -> None:
        while (len(out) + 16) % n:
            out.append(0)

    for index, fields in enumerate(classes):
        record = records[index] if records else murmur3(b"app.Invented")
        out += struct.pack("<I", murmur3(b"root"))
        out += struct.pack("<II", len(fields), record)
        for name, text in fields:
            out += struct.pack("<Ii", murmur3(name.encode()), 15)
            pad(4)
            units = text.encode("utf-16-le")
            out += struct.pack("<I", len(units) // 2) + units
            pad(4)
    pad(8)
    return bytes(out)
