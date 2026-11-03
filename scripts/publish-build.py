"""Publish application files only; competition materials and ZIPs stay local."""
import json
import os
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request

repo = os.environ['GITHUB_REPOSITORY']
sha = os.environ['GITHUB_SHA']
tag = os.environ['RELEASE_TAG']
preview = tag == 'preview'
headers = {'Authorization': 'Bearer ' + os.environ['GITHUB_TOKEN'],
           'Accept': 'application/vnd.github+json', 'User-Agent': 'OmniView-build-publisher'}

def request(method, path, data=None, binary=False):
    url = path if path.startswith('https://') else f'https://api.github.com/repos/{repo}/{path}'
    h = dict(headers)
    if data is not None and not binary:
        data = json.dumps(data).encode()
        h['Content-Type'] = 'application/json'
    elif binary:
        h['Content-Type'] = 'application/octet-stream'
    with urllib.request.urlopen(urllib.request.Request(url, data=data, headers=h, method=method), timeout=120) as r:
        body = r.read()
        return json.loads(body) if body else None

payload = {'tag_name': tag, 'target_commitish': sha, 'name': f'Test build {sha[:7]}' if preview else tag,
           'body': f'Application build from commit {sha}.\n\nDownload the files for one architecture together. No competition documents or presentations are included.',
           'draft': False, 'prerelease': preview}
try:
    release = request('GET', 'releases/tags/' + urllib.parse.quote(tag))
except urllib.error.HTTPError as e:
    if e.code != 404:
        raise
    release = None

files = sorted(Path('publish').glob('*/*'))
files = [p for p in files if p.is_file() and p.name in ('OmniView.exe', 'OmniView-Student.exe', 'OmniView', 'OmniView-Student', 'OmniView-Legacy.dmg')]
assert len(files) == 8, 'Expected Windows x86/x64, Linux x64, macOS Intel/ARM application files'
if release is None:
    release = request('POST', 'releases', {**payload, 'draft': True})
for path in files:
    arch = {'update-x86':'win32','update-x64':'win64'}.get(path.parent.name, path.parent.name.removeprefix('update-'))
    name = arch + '-' + path.name
    for old in release['assets']:
        if old['name'] == name:
            request('DELETE', 'releases/assets/' + str(old['id']))
    upload = release['upload_url'].split('{')[0] + '?name=' + urllib.parse.quote(name)
    asset = request('POST', upload, path.read_bytes(), True)
    assert asset.get('digest', '').startswith('sha256:'), 'GitHub did not return an asset digest'
if preview:
    # Draft releases do not create a tag until publication. Check the ref first;
    # PATCH of a missing ref returns 422 rather than 404 on GitHub.
    try:
        request('GET', 'git/ref/tags/preview')
    except urllib.error.HTTPError as e:
        if e.code != 404:
            raise
        request('POST', 'git/refs', {'ref': 'refs/tags/preview', 'sha': sha})
    else:
        request('PATCH', 'git/refs/tags/preview', {'sha': sha, 'force': True})
request('PATCH', 'releases/' + str(release['id']), payload)
print(release['html_url'])
