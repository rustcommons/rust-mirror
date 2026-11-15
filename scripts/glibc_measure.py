#!/usr/bin/env python3
"""CI-only GNU baseline/old-userland measurements. Never publishes an artifact."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import time
import uuid

ALMA8 = 'almalinux:8@sha256:2d0f2eb2a83cc09dbc2eb92fade4dbe8f709628149e81aa00aec70630406603b'
IMAGES = [ALMA8, 'centos:7', 'opensuse/leap:42.3', 'ubuntu:20.04', 'ubuntu:18.04',
          'ubuntu:16.04', 'debian:9', 'debian:8', 'debian:7', 'centos:6']


def execute(args, log, cwd=None, env=None, timeout=900):
    start = time.monotonic()
    try:
        p = subprocess.run([str(a) for a in args], cwd=cwd, env=env, text=True,
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        text, code = p.stdout, p.returncode
    except subprocess.TimeoutExpired as error:
        text = (error.stdout or b'').decode(errors='replace') if isinstance(error.stdout, bytes) else (error.stdout or '')
        text += '\nmeasurement timeout'; code = 124
    log.parent.mkdir(parents=True, exist_ok=True); log.write_text(text)
    return {'exit': code, 'seconds': round(time.monotonic() - start, 3), 'output': text}


def tail(result):
    return result['output'][-5000:]


class Containers:
    def __init__(self, output):
        self.output = output; self.cache = {}; self.owner = 'rust-mirror-glibc-' + uuid.uuid4().hex[:16]

    def pull(self, image):
        if image not in self.cache:
            n = len(self.cache)
            result = execute(['docker', 'pull', '--platform', 'linux/amd64', image], self.output / f'image-{n}-pull.log', timeout=300)
            info = {'image': image, 'available': result['exit'] == 0, 'failure_reason': tail(result) if result['exit'] else None}
            if info['available']:
                obj = json.loads(subprocess.check_output(['docker', 'image', 'inspect', image], text=True))[0]
                info.update(id=obj['Id'], repo_digests=obj.get('RepoDigests', []), architecture=obj['Architecture'])
            self.cache[image] = info
        return self.cache[image]

    def run(self, image_id, command, log, mounts=(), root=False, timeout=240):
        name = self.owner + '-' + uuid.uuid4().hex[:8]
        args = ['docker', 'run', '--rm', '--name', name, '--label', 'rust-mirror-measure=' + self.owner,
                '--platform', 'linux/amd64', '--security-opt', 'no-new-privileges']
        if not root:
            args += ['--cap-drop', 'ALL', '--user', f'{os.getuid()}:{os.getgid()}', '--read-only', '--tmpfs', '/tmp:rw,nosuid,nodev']
        for source, destination, readonly in mounts:
            args += ['--mount', f'type=bind,src={source},dst={destination}' + (',readonly' if readonly else '')]
        args += ['--entrypoint', command[0], image_id, *command[1:]]
        try:
            return execute(args, log, timeout=timeout)
        finally:
            # A timed-out CLI can leave its container running. Remove only our unique labeled instance.
            check = subprocess.run(['docker', 'inspect', '--format', '{{ index .Config.Labels "rust-mirror-measure" }}', name], capture_output=True, text=True)
            if check.returncode == 0 and check.stdout.strip() == self.owner:
                subprocess.run(['docker', 'rm', '-f', name], check=True, capture_output=True)


def versioned_symbols(binary):
    text = subprocess.check_output(['readelf', '--version-info', binary], text=True)
    values = sorted(set(re.findall(r'GLIBC_([0-9]+\.[0-9]+(?:\.[0-9]+)?)', text)), key=lambda v: tuple(map(int, v.split('.'))))
    dynamic = subprocess.check_output(['readelf', '--dynamic', binary], text=True)
    return {'required_glibc_versions': values, 'maximum_required_glibc': values[-1] if values else None,
            'needed': re.findall(r'\(NEEDED\).*\[([^]]+)\]', dynamic), 'dynamic_section': dynamic}


def smoke(engine, image, binary, output, version):
    output.mkdir(parents=True, exist_ok=False)
    info = engine.pull(image)
    row = {'image': image, 'image_record': info, 'version_ok': False, 'plan_ok': False, 'sync_ok': False}
    if not info['available']:
        return dict(row, result='not_run', failure_stage='image_pull', failure_reason=info['failure_reason'])
    probe = engine.run(info['id'], ['/bin/sh', '-c', 'getconf GNU_LIBC_VERSION'], output / 'userland.log')
    row['runtime_probe'] = probe['output'].strip()
    if probe['exit']:
        return dict(row, result='not_run', failure_stage='image_runtime', failure_reason=tail(probe))
    mounts = [(binary, '/rust-mirror', True)]
    check = engine.run(info['id'], ['/rust-mirror', '--version'], output / 'version.log', mounts)
    if check['exit'] or check['output'].strip() != f'rust-mirror {version}':
        return dict(row, result='failed', failure_stage='version', failure_reason=tail(check), exit=check['exit'])
    row['version_ok'] = True
    state = output / 'state'; state.mkdir()
    catalog = output / 'catalog.toml'
    catalog.write_text('[crates]\nitoa="=1.0.15"\n[crate-policy]\nhosts=["x86_64-unknown-linux-gnu"]\ntargets=["x86_64-unknown-linux-gnu"]\n[crate-output]\nbase-url="https://mirror.example/measurement"\n[crate-network]\ncontact="https://github.com/rustcommons/rust-mirror/issues"\n')
    mounts += [(state, '/out', False), (catalog, '/catalog.toml', True)]
    command = ['/rust-mirror', '--events', 'jsonl', 'crates', 'sync', '--config', '/catalog.toml', '--output', '/out/run']
    preview = engine.run(info['id'], command, output / 'plan.log', mounts)
    try:
        last = json.loads(preview['output'].splitlines()[-1])
        if preview['exit'] or not last.get('ok') or last['report']['status'] != 'needs_approval': raise ValueError('preview refused')
        plan_id = last['report']['plan_id']
    except (ValueError, KeyError, IndexError):
        return dict(row, result='failed', failure_stage='plan_tls', failure_reason=tail(preview), exit=preview['exit'])
    row['plan_ok'] = True
    completed = engine.run(info['id'], [*command, '--approve-plan', plan_id], output / 'sync.log', mounts)
    try:
        last = json.loads(completed['output'].splitlines()[-1])
        if completed['exit'] or not last.get('ok') or last['report']['status'] != 'completed': raise ValueError('sync refused')
    except (ValueError, KeyError, IndexError):
        return dict(row, result='failed', failure_stage='sync_tls', failure_reason=tail(completed), exit=completed['exit'])
    return dict(row, result='passed', sync_ok=True, plan_id=plan_id, failure_stage=None, failure_reason=None,
                dns_tls_hosts=['index.crates.io', 'static.crates.io'], tls_verification=True)


def baseline(args):
    source = args.source.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    engine = Containers(args.output); base = engine.pull(ALMA8)
    if not base['available']: raise SystemExit(base['failure_reason'])
    compiler = Path(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip())
    cache = args.output / 'cargo-cache'; cache.mkdir()
    # Python remains on the host for packaging/tomllib; python39 records the container's own toolchain.
    script = '''set -eu
    dnf install -y git gcc binutils python39 tar gzip findutils which
    git config --global --add safe.directory /work/source
    export PATH=/opt/rust/bin:$PATH
    export CARGO_HOME=/work/cargo-cache
    export CARGO_BUILD_JOBS=2
    cd /work/source
    cargo build --release --locked --target x86_64-unknown-linux-gnu
    /usr/bin/python3.9 -c 'import json,subprocess; print("BASELINE_METADATA="+json.dumps({"glibc":subprocess.check_output(["getconf","GNU_LIBC_VERSION"],text=True).strip(),"gcc":subprocess.check_output(["gcc","--version"],text=True).splitlines()[0],"rustc":subprocess.check_output(["rustc","--version"],text=True).strip(),"python":"3.9"}))'
    '''
    result = engine.run(base['id'], ['/bin/sh', '-c', script], args.output / 'build.log', [(source, '/work/source', False), (compiler, '/opt/rust', True), (cache, '/work/cargo-cache', False)], root=True, timeout=1500)
    if result['exit']: raise SystemExit(tail(result))
    binary = source / 'target/x86_64-unknown-linux-gnu/release/rust-mirror'
    metadata = json.loads(next(line.split('=', 1)[1] for line in result['output'].splitlines() if line.startswith('BASELINE_METADATA=')))
    assert metadata['glibc'] == 'glibc 2.28'
    linkage = versioned_symbols(str(binary)); assert linkage['maximum_required_glibc'] and tuple(map(int, linkage['maximum_required_glibc'].split('.'))) <= (2, 28)
    row = smoke(engine, ALMA8, binary, args.output / 'alma8-smoke', args.version)
    report = {'method': 'almalinux8', 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source, text=True).strip(), 'base_image': base, 'toolchain': metadata, 'linkage': linkage, 'runtime': row, 'host_kernel': platform.release(), 'container_kernel_scope': 'containers share CI host kernel; not native old-kernel qualification'}
    (args.output / 'baseline-report.json').write_text(json.dumps(report, indent=2) + '\n')
    if row['result'] != 'passed': raise SystemExit('AlmaLinux8 runtime/real DNS/TLS sync failed: ' + str(row['failure_reason']))
    print(json.dumps(report, indent=2))


def experiments(args):
    args.output.mkdir(parents=True, exist_ok=False); engine = Containers(args.output)
    source = args.source.resolve(); tool = args.tool.resolve()
    env = dict(os.environ); env['PATH'] = str(tool) + os.pathsep + env['PATH']; env['CARGO_BUILD_JOBS'] = '2'
    env.pop('RUSTFLAGS', None); env.pop('CARGO_ENCODED_RUSTFLAGS', None)
    report = {'measurement_script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'workflow_sha256': hashlib.sha256((Path(__file__).resolve().parents[1] / '.github/workflows/glibc-experiments.yml').read_bytes()).hexdigest(), 'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source, text=True).strip(), 'cargo_libc_commit': args.tool_commit, 'cargo_libc_version': subprocess.check_output([str(tool / 'cargo-libc'), '--version'], text=True).strip(), 'host_kernel': platform.release(), 'ci_only': True, 'builds': [], 'results': [], 'limits': ['Containers share the host kernel; no native old-kernel proof', 'No commercial SLES image/EULA used; openSUSE Leap42.3 is a community SLE12-era proxy']}
    if args.baseline:
        for n, image in enumerate(IMAGES):
            row = smoke(engine, image, args.baseline.resolve(), args.output / f'baseline-userland-{n}', args.version)
            report['results'].append(dict(row, method='almalinux8', target_glibc='2.28-build-baseline'))
        (args.output / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
    # The first unsupported lower boundary is measured too, never silently extrapolated.
    for target in ('2.28', '2.17', '2.16'):
        work = args.output / ('target-' + target); work.mkdir()
        local_env = dict(env, CARGO_TARGET_DIR=str(work / 'build'))
        binary = work / 'build/x86_64-unknown-linux-gnu/release/rust-mirror'
        command = ['cargo', 'libc', 'build', '--target', 'x86_64-unknown-linux-gnu.' + target, '--cc', '/usr/bin/clang-18', '--artifact', str(binary), '--', '--release', '--locked']
        built = execute(command, work / 'build.log', cwd=source, env=local_env, timeout=1500)
        item = {'target_glibc': target, 'command': command, 'exit': built['exit'], 'result': 'passed' if built['exit'] == 0 else 'failed', 'failure_reason': tail(built) if built['exit'] else None}
        if built['exit'] == 0: item['linkage'] = versioned_symbols(str(binary))
        report['builds'].append(item)
        for n, image in enumerate(IMAGES):
            if built['exit']:
                row = {'image': image, 'result': 'not_run', 'version_ok': False, 'plan_ok': False, 'sync_ok': False, 'failure_stage': 'build', 'failure_reason': item['failure_reason']}
            else:
                row = smoke(engine, image, binary, work / f'userland-{n}', args.version)
            report['results'].append(dict(row, method='cargo-libc', target_glibc=target))
        (args.output / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
    report['images'] = list(engine.cache.values())
    (args.output / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
    print('Measured combinations:', len(report['results']))


def main():
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('mode', choices=['baseline', 'experiments'])
    p.add_argument('--source', type=Path, required=True); p.add_argument('--output', type=Path, required=True); p.add_argument('--version', default='0.5.1')
    p.add_argument('--baseline', type=Path); p.add_argument('--tool', type=Path); p.add_argument('--tool-commit')
    args = p.parse_args()
    if args.mode == 'baseline': baseline(args)
    else:
        if not args.tool or not args.tool_commit: p.error('experiments require pinned tool path/commit')
        experiments(args)


if __name__ == '__main__': main()
