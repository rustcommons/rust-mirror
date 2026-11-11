#!/usr/bin/env python3
"""Prepare genuine old/new Rustup generations for bounded native qualification."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--cli",type=Path,required=True)
    p.add_argument("--host",required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--previous",type=Path)
    p.add_argument("--extra-channels", action="store_true", help="Include current beta/nightly in the new generation")
    args=p.parse_args()
    root=args.output.resolve();root.mkdir(parents=True,exist_ok=False)
    cli=args.cli.resolve()
    if not args.host or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-_" for c in args.host): raise ValueError("invalid host")
    records=[]
    for name,date,version in [("old","2025-08-07","1.28.2"),("new","2025-09-18","1.29.3")]:
        config=root/(name+".toml")
        text=f'schema = 1\nrustup_version = "1.28.2"\nmax_download_bytes = 10737418240\n[[selections]]\nname = "native-lifecycle"\ntoolchain = "stable-{date}"\nhosts = ["{args.host}"]\nprofile = "minimal"\n'
        if name=="new":
            text=text.replace('profile = "minimal"','profile = "default"')
            text+='components = ["rust-src"]\ntargets = ["wasm32-unknown-unknown"]\n'
        if name=="new" and args.extra_channels:
            for channel in ["beta", "nightly"]:
                text+=f'[[selections]]\nname="{channel}-native"\ntoolchain="{channel}"\nhosts=["{args.host}"]\nprofile="minimal"\n'
        config.write_text(text,encoding="utf-8")
        command=[str(cli),"sync","--config",str(config),"--output",str(root/(name+"-raw"))]
        previous=root/"old-raw/mirror" if name=="new" else args.previous
        if previous: command.extend(["--previous",str(previous.resolve())])
        for phase,cmd in [("sync",command),("rustup",[str(cli),"rustup","--snapshot",str(root/(name+"-raw/mirror")),"--version",version,"--promote","stable-"+date,"--output",str(root/name)])]:
            with (root/(name+"-"+phase+".stdout")).open("wb") as out,(root/(name+"-"+phase+".stderr")).open("wb") as err:
                result=subprocess.run(cmd,stdout=out,stderr=err,timeout=1800)
            records.append({"phase":name+"-"+phase,"exit":result.returncode})
            (root/"preparation.json").write_text(json.dumps(records,indent=2)+"\n",encoding="utf-8")
            if result.returncode: raise RuntimeError("preparation failed: "+name+"-"+phase)
        print(name+" generation ready",flush=True)

if __name__=="__main__":main()
