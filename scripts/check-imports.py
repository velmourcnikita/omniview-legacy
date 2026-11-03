"""Reject known newer-Windows static imports. This is not an OS compatibility test."""
import json
from pathlib import Path
import struct
import subprocess
import sys

p=Path(sys.argv[1])
imports=subprocess.check_output(['llvm-readobj','--coff-imports',str(p)],text=True)
for symbol in ['ProcessPrng','WaitOnAddress','WakeByAddress','SetThreadDescription','GetSystemTimePreciseAsFileTime','WebView2Loader','api-ms-win-crt','VCRUNTIME140']:
    assert symbol.lower() not in imports.lower(), f'Unsupported static import: {symbol}'
b=p.read_bytes();pe=struct.unpack_from('<I',b,0x3c)[0];optional=pe+24
assert struct.unpack_from('<HH',b,optional+48)==(6,1), 'Wrong subsystem version'
assert struct.unpack_from('<H',b,pe+4)[0] in (0x14c,0x8664)
print(json.dumps({'binary':str(p),'bytes':len(b),'subsystem':'6.1','known_incompatible_imports':False,'physical_windows7_test':'pending'}))
