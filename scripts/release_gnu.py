#!/usr/bin/env python3
"""Build the owner-selected GNU artifact and fail closed on every old-userland gate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
from glibc_measure import ALMA8, Containers, execute, smoke, tail, versioned_symbols

CARGO_LIBC_COMMIT = 'e68ca7bf3a8ffd973c8576acc48abd0fbcc66d25'
GATES = [
    ALMA8,
    'centos:7@sha256:be65f488b7764ad3638f236b7b515b3678369a5124c47b8d32916d6487418ea4',
    'opensuse/leap:42.3@sha256:271c921a1c84d65467d565e3f1c754b75aa01ceed1a7ac39de4816683c355fd2',
    'ubuntu:20.04@sha256:8feb4d8ca5354def3d8fce243717141ce31e2c428701f6682bd2fafe15388214',
    'ubuntu:18.04@sha256:152dc042452c496007f07ca9127571cb9c29697f42acbfad72324b2bb2e43c98',
    'ubuntu:16.04@sha256:1f1a2d56de1d604801a9671f301190704c25d604a416f59e03c04f5c6ffee0d6',
    'debian:9@sha256:c5c5200ff1e9c73ffbf188b4a67eb1c91531b644856b4aefe86a58d2f0cb05be',
    'debian:8@sha256:32ad5050caffb2c7e969dac873bce2c370015c2256ff984b70c1c08b3a2816a0',
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--tool', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', required=True)
    args = parser.parse_args()
    source = args.source.resolve(); tool = args.tool.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, CARGO_BUILD_JOBS='2', CARGO_TARGET_DIR=str(source / 'target'))
    env['PATH'] = str(tool) + os.pathsep + env['PATH']
    env.pop('RUSTFLAGS', None); env.pop('CARGO_ENCODED_RUSTFLAGS', None)
    version = subprocess.check_output([str(tool / 'cargo-libc'), '--version'], text=True).strip()
    if version != 'cargo-libc 0.7.0': raise SystemExit('GNU release requires qualified cargo-libc 0.7.0')
    binary = source / 'target/x86_64-unknown-linux-gnu/release/rust-mirror'
    command = ['cargo', 'libc', 'build', '--target', 'x86_64-unknown-linux-gnu.2.17',
               '--cc', '/usr/bin/clang-18', '--artifact', str(binary), '--', '--release', '--locked']
    built = execute(command, args.output / 'build.log', cwd=source, env=env, timeout=1500)
    if built['exit']: raise SystemExit(tail(built))
    linkage = versioned_symbols(str(binary))
    maximum = linkage['maximum_required_glibc']
    if maximum is None or tuple(map(int, maximum.split('.'))) > (2, 17): raise SystemExit('GNU artifact exceeds glibc 2.17')
    engine = Containers(args.output)
    report = {'method': 'cargo-libc', 'target_glibc': '2.17', 'cargo_libc_commit': CARGO_LIBC_COMMIT,
              'cargo_libc_version': version, 'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source, text=True).strip(),
              'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'command': command,
              'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
              'clang': subprocess.check_output(['/usr/bin/clang-18', '--version'], text=True).splitlines()[0],
              'gcc': subprocess.check_output(['gcc', '--version'], text=True).splitlines()[0],
              'linkage': linkage, 'host_kernel': platform.release(), 'runtime_gates': [],
              'scope': 'Container userland version and real HTTPS plan/sync; old native kernels not tested'}
    for index, image in enumerate(GATES):
        result = smoke(engine, image, binary, args.output / f'userland-{index}', args.version)
        report['runtime_gates'].append(result)
        (args.output / 'gnu-report.json').write_text(json.dumps(report, indent=2) + '\n')
        if result['result'] != 'passed': raise SystemExit('GNU publication gate failed on ' + image + ': ' + str(result.get('failure_reason')))
    print(json.dumps(report, indent=2))


if __name__ == '__main__': main()
