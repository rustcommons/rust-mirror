#!/usr/bin/env python3
"""Cargo 1.90 oracle for historical collisions. Metadata only; archives are refused."""
import argparse
from collections import Counter
import hashlib
import http.server
import json
from pathlib import Path
import subprocess
import threading
import urllib.request
from check_crates_phase0 import cargo_environment, project, lock_ids
from crates_phase0_fixtures import record, dep


def shard(name):
    return ('1/'+name if len(name)==1 else '2/'+name if len(name)==2 else '3/'+name[0]+'/'+name if len(name)==3 else name[:2]+'/'+name[2:4]+'/'+name)


def check(driver, work, roots, entries=None, refused=False, real_ndk=False, production=False):
    work.mkdir(parents=True)
    cache=work/'index';cache.mkdir()
    if real_ndk:
        raw=(Path(__file__).resolve().parents[1]/'tests/fixtures/ndk-sys-20261114.index').read_bytes()
        assert hashlib.sha256(raw).hexdigest()=='767c4cf0391a7ed161587aa12735e71f138aea5fda8bb5488d13ee3e5ceaf555'
        file=cache/shard('ndk-sys');file.parent.mkdir(parents=True);file.write_bytes(raw)
    for entry in entries or []:
        file=cache/shard(entry['name']);file.parent.mkdir(parents=True,exist_ok=True)
        with file.open('ab') as out:out.write((json.dumps(entry)+'\n').encode())
    rootfile=work/'roots.json';rootfile.write_text(json.dumps(roots));planfile=work/'plan.json'
    native=subprocess.run([str(driver),str(rootfile),str(cache),str(planfile),*(['--offline'] if entries else []),*(['--production-bounds'] if production else [])],capture_output=True,text=True,timeout=600)
    (work/'native.log').write_text(native.stdout+native.stderr)
    assert (native.returncode!=0)==refused,(work.name,native.stderr[-3000:])
    filtered={};selected=set();warnings=[]
    if not refused:
        plan=json.loads(planfile.read_text());assert plan['archive_requests']==0
        warnings=plan.get('index_warnings',[])
        selected={(v['name'],v['version']) for v in plan['versions']}
        exact=Counter(bytes(v['index_line']) for v in plan['versions'])
        for file in cache.rglob('*'):
            if file.is_file():
                raw=file.read_bytes()
                lines=[]
                for line in raw.splitlines(keepends=True):
                    if exact[line]:
                        lines.append(line);exact[line]-=1
                if lines:filtered[file.relative_to(cache).as_posix()]=b''.join(lines)
        assert not +exact, 'selected raw lines missing from source index'
        for v in plan['versions']:
            assert bytes(v['index_line']) in filtered[shard(v['name'])].splitlines(keepends=True)
        assert any(w['name']=='ndk-sys' if real_ndk else w['name'] in ('x','ndk-sys') for w in warnings)
    else:
        assert 'index version collision' in native.stderr and 'historical' in native.stderr
        assert '0'*64 in native.stderr and '1'*64 in native.stderr
    archives=[]
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}),
        type('NoRedirect',(urllib.request.HTTPRedirectHandler,),{'redirect_request':lambda *a,**kw:None})())
    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self,*args):pass
        def do_GET(self):
            if self.path.startswith('/archive/'):
                archives.append(self.path);self.send_error(403);return
            mode,_,path=self.path.lstrip('/').partition('/')
            if mode not in ('full','filtered') or not path or '..' in path.split('/') or '?' in path:
                self.send_error(400);return
            if path=='config.json':data=json.dumps({'dl':base+'/archive/{crate}/{version}'}).encode()
            elif mode=='filtered':data=filtered.get(path)
            else:
                file=cache/path
                if not file.exists() and entries is None:
                    request=urllib.request.Request('https://index.crates.io/'+path,headers={'User-Agent':'rust-mirror-conformance (https://github.com/rustcommons/rust-mirror)'})
                    with opener.open(request,timeout=45) as r:data=r.read(32*1024*1024+1)
                    assert len(data)<=32*1024*1024
                    file.parent.mkdir(parents=True,exist_ok=True);file.write_bytes(data)
                data=file.read_bytes() if file.exists() else None
            if data is None:self.send_error(404);return
            self.send_response(200);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler);base=f'http://127.0.0.1:{server.server_port}'
    worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start();resolved=[]
    try:
        for n,root in enumerate(roots):
            choices={}
            for mode in (['full'] if refused else ['full','filtered']):
                path=work/f'{n}-{mode}';project(path,root,base,mode)
                result=subprocess.run(['cargo','generate-lockfile'],cwd=path,env=cargo_environment(work/f'cargo-{n}-{mode}'),capture_output=True,text=True,timeout=600)
                (work/f'{n}-{mode}.log').write_text(result.stdout+result.stderr)
                assert result.returncode==0,(work.name,mode,result.stderr[-3000:])
                choices[mode]=sorted(lock_ids(path))
                if mode=='filtered':assert set(choices[mode])<={a+'@'+b for a,b in selected}
            resolved.append({'root':root,'choices':choices})
    finally:
        server.shutdown();server.server_close();worker.join(timeout=10)
    assert not archives
    report={'case':work.name,'native_selected_collision_refusal':refused,'cargo_resolved':resolved,'index_warnings':warnings,'archive_requests':archives,'raw_line_preservation':not refused,'cargo':subprocess.check_output(['cargo','--version'],text=True).strip()}
    (work/'summary.json').write_text(json.dumps(report,indent=2)+'\n');return report


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--driver',required=True,type=Path);p.add_argument('--work',required=True,type=Path);args=p.parse_args()
    driver=args.driver.resolve();work=args.work.resolve();work.mkdir(parents=True,exist_ok=False);reports=[]
    for yanked in [False,True]:
        entries=[record('root',deps=[dep('x','2')]),record('x'),record('x','1.0.0+historical',cksum='1'*64,yanked=yanked),record('x','2.0.0')]
        reports.append(check(driver,work/('historical-yanked' if yanked else 'historical-live'),[{'name':'root','version':'1.0.0'}],entries))
    entries=[record('root',deps=[dep('x','1')]),record('x'),record('x','1.0.0+historical',cksum='1'*64,yanked=True)]
    bevy=[record('bevy','0.15.3',deps=[dep('winit','0.30')]),record('winit','0.30.12',deps=[dep('ndk-sys','0.6',target='cfg(target_os = "android")')]),record('ndk-sys','0.4.0'),record('ndk-sys','0.4.0+historical',cksum='1'*64,yanked=True),record('ndk-sys','0.6.0')]
    reports.append(check(driver,work/'bevy-target-style',[{'name':'bevy','version':'0.15.3'}],bevy))
    reports.append(check(driver,work/'selected-refusal',[{'name':'root','version':'1.0.0'}],entries,refused=True))
    reports.append(check(driver,work/'real-ndk',[{'name':'ndk-sys','version':'0.6.0+11769913'}],real_ndk=True))
    reports.append(check(driver,work/'windowing-roots',[{'name':'winit','version':'0.30.12'},{'name':'eframe','version':'0.31.1'}],real_ndk=True,production=True))
    (work/'summary.json').write_text(json.dumps(reports,indent=2)+'\n')
    print('Collision conformance passed:',len(reports),'cases; no archive requests')

if __name__=='__main__':main()
