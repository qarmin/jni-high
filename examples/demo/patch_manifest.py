#!/usr/bin/env python3
"""
Patches an Android APK's binary manifest (AXML) to add
android:extractNativeLibs="true" to the <application> element.

Required for ASAN wrap.sh: cargo-apk ignores extract_native_libs in Cargo.toml,
so the attribute must be injected after the APK is built.

Usage: python3 patch_manifest.py <apk_path>
Modifies the APK in-place.
"""
import struct, sys, zipfile, os

def u32(d, o): return struct.unpack_from('<I', d, o)[0]
def u16(d, o): return struct.unpack_from('<H', d, o)[0]
def p32(d, o, v): struct.pack_into('<I', d, o, v)
def p16(d, o, v): struct.pack_into('<H', d, o, v)

ANDROID_NS = 'http://schemas.android.com/apk/res/android'
EXTRACT_RES_ID = 0x010104ea  # android:extractNativeLibs (API 23, AOSP public.xml)
UTF8_FLAG = 1 << 8

# --- String pool readers/encoders (UTF-8 and UTF-16) ---

def varint_read(d, o):
    v = d[o]; o += 1
    if v & 0x80: v = ((v & 0x7f) << 8) | d[o]; o += 1
    return v, o

def varint_write(n):
    return bytes([n]) if n < 0x80 else bytes([0x80 | (n >> 8), n & 0xff])

def read_utf8_str(d, abs_off):
    o = abs_off
    _, o = varint_read(d, o)        # utf-16 char count (skip)
    u8l, o = varint_read(d, o)      # utf-8 byte count
    s = bytes(d[o:o+u8l]).decode('utf-8', errors='replace')
    return s, o + u8l + 1 - abs_off # (text, bytes consumed incl. null)

def encode_utf8_str(s):
    b = s.encode('utf-8')
    return varint_write(len(s)) + varint_write(len(b)) + b + b'\x00'

def read_utf16_str(d, abs_off):
    n = u16(d, abs_off)             # number of UTF-16 code units
    start = abs_off + 2
    s = bytes(d[start:start + n * 2]).decode('utf-16-le', errors='replace')
    return s, 2 + n * 2 + 2        # (text, bytes consumed incl. null)

def encode_utf16_str(s):
    b = s.encode('utf-16-le')
    n = len(s)                      # number of UTF-16 code units
    return struct.pack('<H', n) + b + b'\x00\x00'


def patch_manifest(raw: bytes) -> bytes:
    d = bytearray(raw)

    assert u32(d, 0) == 0x00080003, f"Not AXML (magic={u32(d,0):#010x})"

    SP = 8  # string pool immediately follows 8-byte XML file header
    assert u16(d, SP) == 0x0001, "Expected string pool chunk"

    sp_size   = u32(d, SP + 4)
    str_count = u32(d, SP + 8)
    flags     = u32(d, SP + 16)
    str_off   = u32(d, SP + 20)  # offset within SP chunk to string data
    offsets   = SP + 28          # string offset table

    is_utf8 = bool(flags & UTF8_FLAG)

    def abs_str(i): return SP + str_off + u32(d, offsets + i * 4)

    def read_str(i):
        if is_utf8:
            s, _ = read_utf8_str(d, abs_str(i))
        else:
            s, _ = read_utf16_str(d, abs_str(i))
        return s

    def encode_str(s):
        return encode_utf8_str(s) if is_utf8 else encode_utf16_str(s)

    android_ns_idx  = next((i for i in range(str_count) if ANDROID_NS in read_str(i)), None)
    application_idx = next((i for i in range(str_count) if read_str(i) == 'application'), None)
    extract_idx     = next((i for i in range(str_count) if read_str(i) == 'extractNativeLibs'), None)

    assert android_ns_idx  is not None, "android namespace URL not found in string pool"
    assert application_idx is not None, "'application' string not found in string pool"

    new_string_added = False
    if extract_idx is None:
        new_str_bytes = encode_str('extractNativeLibs')
        extract_idx   = str_count

        # New string's offset within string data = last string's end
        last_abs = abs_str(str_count - 1)
        if is_utf8:
            _, last_sz = read_utf8_str(d, last_abs)
        else:
            _, last_sz = read_utf16_str(d, last_abs)
        new_pool_off = u32(d, offsets + (str_count - 1) * 4) + last_sz

        # Insert 4-byte offset entry at end of offset table
        ins1 = offsets + str_count * 4
        d = d[:ins1] + bytearray(struct.pack('<I', new_pool_off)) + d[ins1:]

        # String data shifted by 4 bytes; update the strings_start field
        str_off += 4
        p32(d, SP + 20, str_off)

        # Append new string at end of string pool (after the +4 offset table shift)
        sp_end = SP + sp_size + 4
        d = d[:sp_end] + bytearray(new_str_bytes) + d[sp_end:]

        added = 4 + len(new_str_bytes)
        p32(d, SP + 8, str_count + 1)
        p32(d, SP + 4, sp_size + added)
        p32(d, 4, u32(d, 4) + added)

        str_count += 1
        sp_size   += added
        new_string_added = True

    # Resource map immediately follows the string pool.
    RM = SP + sp_size
    rm_size = 0
    if u16(d, RM) == 0x0180:
        rm_size  = u32(d, RM + 4)
        rm_count = (rm_size - 8) // 4

        if new_string_added:
            # Extend the resource map from rm_count entries to (extract_idx + 1) entries.
            # Strings in the gap (rm_count .. extract_idx-1) have no resource ID (0x00000000).
            # The new string at extract_idx gets EXTRACT_RES_ID.
            gap = extract_idx - rm_count  # zero-padded entries needed before our new one
            new_entries = bytearray((gap + 1) * 4)  # all zeros
            struct.pack_into('<I', new_entries, gap * 4, EXTRACT_RES_ID)
            added_rm = len(new_entries)
            d = d[:RM + rm_size] + new_entries + d[RM + rm_size:]
            p32(d, RM + 4, rm_size + added_rm)
            p32(d, 4, u32(d, 4) + added_rm)
            rm_size += added_rm

    # Scan XML node stream for the <application> START_ELEMENT
    off = RM + rm_size
    file_size = u32(d, 4)
    while off < file_size:
        chunk_type = u16(d, off)
        chunk_size = u32(d, off + 4)
        if chunk_size == 0:
            break

        if chunk_type == 0x0102:  # START_ELEMENT
            name_idx   = u32(d, off + 20)
            attr_start = u16(d, off + 24)
            attr_each  = u16(d, off + 26)
            attr_count = u16(d, off + 28)
            attrs_abs  = off + 16 + attr_start

            if name_idx == application_idx:
                for ai in range(attr_count):
                    if u32(d, attrs_abs + ai * attr_each + 4) == extract_idx:
                        print("extractNativeLibs already present in <application>")
                        return bytes(d)

                # 20-byte attribute: ns_idx, name_idx, raw=-1, typed_value(boolean true)
                new_attr = struct.pack('<IIiHBBI',
                    android_ns_idx, extract_idx, -1,
                    0x0008, 0x00, 0x12, 0x00000001)

                ins3 = attrs_abs + attr_count * attr_each
                d = d[:ins3] + bytearray(new_attr) + d[ins3:]

                p16(d, off + 28, attr_count + 1)
                p32(d, off + 4, chunk_size + 20)
                p32(d, 4, u32(d, 4) + 20)

                print('Added android:extractNativeLibs="true" to <application>')
                return bytes(d)

        off += chunk_size

    raise RuntimeError("<application> element not found in manifest")


def main():
    apk = sys.argv[1]
    with zipfile.ZipFile(apk, 'r') as z:
        original = z.read('AndroidManifest.xml')

    patched = patch_manifest(original)
    if patched == original:
        print("Manifest already patched, nothing to do")
        return

    tmp = apk + '.patching'
    with zipfile.ZipFile(apk, 'r') as zin, zipfile.ZipFile(tmp, 'w') as zout:
        for info in zin.infolist():
            data = patched if info.filename == 'AndroidManifest.xml' else zin.read(info.filename)
            zout.writestr(info, data)
    os.replace(tmp, apk)
    print(f"APK patched: {apk}")


main()
