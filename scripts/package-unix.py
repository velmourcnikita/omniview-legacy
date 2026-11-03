"""Build a Linux executable pair or macOS application disk image."""
from pathlib import Path
import os
import plistlib
import shutil
import subprocess
import sys

platform = sys.argv[1]
target = sys.argv[2]
out = Path('dist') / platform
out.mkdir(parents=True, exist_ok=True)
source = Path('target') / target / 'release/omniview'
if platform.startswith('linux-'):
    for name in ['OmniView', 'OmniView-Student']:
        shutil.copyfile(source, out / name)
        (out / name).chmod(0o755)
else:
    staging = Path('target') / ('bundle-' + platform)
    for name in ['OmniView Legacy', 'OmniView Legacy Student']:
        contents = staging / (name + '.app') / 'Contents'
        (contents / 'MacOS').mkdir(parents=True, exist_ok=True)
        binary = 'OmniView-Student' if 'Student' in name else 'OmniView'
        shutil.copyfile(source, contents / 'MacOS' / binary)
        (contents / 'MacOS' / binary).chmod(0o755)
        plist = {'CFBundleExecutable': binary, 'CFBundleIdentifier': 'org.omnicontrol.omniviewlegacy' + ('.student' if 'Student' in name else ''),
                 'CFBundleName': name, 'CFBundlePackageType': 'APPL', 'CFBundleShortVersionString': '0.2.0',
                 'CFBundleVersion': '2', 'NSHighResolutionCapable': True, 'LSMinimumSystemVersion': '11.0',
                 'NSHumanReadableCopyright': 'OmniCONTROL'}
        (contents / 'Info.plist').write_bytes(plistlib.dumps(plist))
        subprocess.run(['codesign', '--force', '--deep', '--sign', '-', str(contents.parent)], check=True)
    subprocess.run(['hdiutil', 'create', '-ov', '-volname', 'OmniView Legacy', '-srcfolder', str(staging), str(out / 'OmniView-Legacy.dmg')], check=True)
shutil.copyfile('README.md', out / 'README.md')
shutil.copyfile('LICENSE', out / 'LICENSE')
shutil.copyfile('assets/OFL.txt', out / 'OFL.txt')
print(out)
