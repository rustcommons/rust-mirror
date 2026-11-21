#!/usr/bin/env python3
"""Compare actual bytes from independent hosted builds; never treat differences as identity."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import release_binaries as release


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def metadata(raw):
    normalized = bytearray(raw)
    result = {}
    if raw.startswith(b'MZ') and len(raw) >= 64:
        pe = struct.unpack_from('<I', raw, 0x3c)[0]
        if pe + 12 <= len(raw) and raw[pe:pe+4] == b'PE\0\0':
            result['pe_coff_timestamp'] = struct.unpack_from('<I', raw, pe + 8)[0]
            normalized[pe+8:pe+12] = b'\0' * 4
    if raw[:4] == b'\xcf\xfa\xed\xfe' and len(raw) >= 32:
        count = struct.unpack_from('<I', raw, 16)[0]
        if count <= 512:
            offset = 32
            for _ in range(count):
                if offset + 8 > len(raw): break
                command, size = struct.unpack_from('<II', raw, offset)
                if size < 8 or offset + size > len(raw): break
                if command == 0x1b and size == 24:
                    result['macho_uuid'] = raw[offset+8:offset+24].hex()
                    normalized[offset+8:offset+24] = b'\0' * 16
                if command == 0x1d and size == 16:
                    start, length = struct.unpack_from('<II', raw, offset + 8)
                    if start + length <= len(raw):
                        result['macho_code_signature_sha256'] = digest(raw[start:start+length])
                        normalized[start:start+length] = b'\0' * length
                offset += size
    result['metadata_normalized_sha256'] = digest(normalized)
    return result


def compare(a, b, version):
    rows = []
    for target in release.TARGETS:
        name = release.asset_name(version, target)
        first = (a / name).read_bytes(); second = (b / name).read_bytes()
        exe = 'rust-mirror.exe' if 'windows' in target else 'rust-mirror'
        prefix = name.removesuffix('.zip').removesuffix('.tar.gz')
        files_a = release.read_archive(a / name, prefix, exe)
        files_b = release.read_archive(b / name, prefix, exe)
        binary_same = files_a[exe] == files_b[exe]
        archive_same = first == second
        diagnostic_a, diagnostic_b = metadata(files_a[exe]), metadata(files_b[exe])
        if archive_same and binary_same:
            reason = 'Bit-identical executable and archive for these two independent clean builds'
        elif files_a == files_b:
            reason = 'Archive container/compression metadata differs; every extracted member is byte-identical'
        elif not binary_same and diagnostic_a['metadata_normalized_sha256'] == diagnostic_b['metadata_normalized_sha256']:
            reason = 'Executable differences are confined to observed PE timestamp or Mach-O UUID/signature metadata; raw bytes are NOT identical'
        else:
            reason = 'Executable or archive-member content differs beyond recognized metadata; root cause requires review (not a reproducibility pass)'
        rows.append({'target': target, 'archive': name, 'executable_identical': binary_same,
                     'archive_identical': archive_same, 'binary_sha256': [digest(files_a[exe]), digest(files_b[exe])],
                     'archive_sha256': [digest(first), digest(second)], 'differing_members': [n for n in files_a if files_a[n] != files_b[n]],
                     'diagnostics': [diagnostic_a, diagnostic_b], 'reason': reason})
    sbom = release.sbom_name(version)
    crate = release.crate_name(version)
    crate_sha256 = [digest((a / crate).read_bytes()), digest((b / crate).read_bytes())]
    return {'version': version, 'rows': rows, 'all_bit_identical': all(r['executable_identical'] and r['archive_identical'] for r in rows),
            'sbom_sha256': [digest((a / sbom).read_bytes()), digest((b / sbom).read_bytes())],
            'crate_sha256': crate_sha256, 'crate_identical': crate_sha256[0] == crate_sha256[1],
            'scope': 'Two fresh hosted jobs per target, no target cache; not a universal reproducibility guarantee. Differences are observations, never silently normalized away.'}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--first', type=Path, required=True)
    p.add_argument('--second', type=Path, required=True)
    p.add_argument('--version', required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args(); result = compare(a.first, a.second, a.version)
    a.output.mkdir(parents=True, exist_ok=False)
    (a.output / 'reproducibility.json').write_text(json.dumps(result, indent=2) + '\n')
    table = '| Target | Executable identical | Archive identical | Observation |\n| --- | --- | --- | --- |\n'
    for r in result['rows']:
        table += f"| {r['target']} | {r['executable_identical']} | {r['archive_identical']} | {r['reason']} |\n"
    table += f"| crate | {result['crate_identical']} | {result['crate_identical']} | Packaged .crate bytes from the same frozen source |\n"
    (a.output / 'reproducibility.md').write_text(table)
    print(table)
    if not result['all_bit_identical']:
        print('::warning::Reproducibility differences recorded; do not claim bit-identical builds. Review the measured causes.')


if __name__ == '__main__':
    main()
