#!/usr/bin/env python3
"""Enforce full action commits plus readable upstream version comments."""
from pathlib import Path
import re
import sys


def check(directory):
    rows=[]
    for path in sorted(directory.glob('*.yml')):
        for number,line in enumerate(path.read_text().splitlines(),1):
            match=re.match(r'\s+(?:-\s+)?uses:\s+([^\s#]+)(.*)',line)
            if not match:continue
            ref,comment=match.groups()
            if ref.startswith(('./','$/' )):continue
            if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[a-f0-9]{40}',ref) or not re.match(r'\s+#\s+v?\d+(?:\.\d+)*(?:[-+][A-Za-z0-9.-]+)?\s*$',comment):
                raise ValueError(f'{path}:{number}: action needs a full commit and trailing version tag comment')
            rows.append({'workflow':str(path),'line':number,'action':ref,'version':comment.strip('# ').strip()})
    if not rows:raise ValueError('no action references checked')
    return rows


if __name__=='__main__':
    try:
        rows=check(Path('.github/workflows'));print(f'{len(rows)} third-party action references use full commits and version comments')
    except ValueError as error:sys.exit(str(error))
