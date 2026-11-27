# -*- coding: utf-8 -*-
"""CHANGELOG.json is the one source of every version's notes; this turns it into what the
public repository shows, so neither can fall behind:

- CHANGELOG.md, every version newest first (English, the French in a fold), written into
  the public snapshot by mirror.py;
- a GitHub release per version (tag v<version>), its notes from the same entry, created or
  updated by mirror.py after it pushes the tag.

    python tools/changelog.py                      print CHANGELOG.md
    python tools/changelog.py --releases           create or update the releases of every
                                                   tag on GitHub that lacks or differs (gh)
    python tools/changelog.py --releases --dry     say what it would do

publish.py already refuses a version without notes in English and French.
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CHANGELOG = os.path.join(ROOT, 'CHANGELOG.json')
REPO = 'ftstp/spool'
DOWNLOAD = 'https://fatstep.com/tools/spool'


def entries():
    return json.load(open(CHANGELOG, encoding='utf-8'))


def _key(v):
    return tuple(int(n) for n in v.split('.'))


def release_body(e):
    out = [f'- {n}' for n in e['notes']['en']]
    fr = e['notes'].get('fr') or []
    if fr:
        out += ['', '<details><summary>En français</summary>', '']
        out += [f'- {n}' for n in fr]
        out += ['', '</details>']
    out += ['', f'Download: [{DOWNLOAD.split("//")[1]}]({DOWNLOAD}). Installed copies offer the update by themselves.']
    return '\n'.join(out) + '\n'


def markdown():
    lines = [
        '# Changelog',
        '',
        f'Every version of Spool, newest first. Download the latest at [{DOWNLOAD.split("//")[1]}]({DOWNLOAD});',
        'installed copies offer each update by themselves.',
        '',
        '*This file is generated from `CHANGELOG.json` at each release.*',
        '',
    ]
    for e in sorted(entries(), key=lambda e: _key(e['version']), reverse=True):
        lines += [f'## {e["version"]} ({e["date"]})', '', release_body(e).rsplit('\n\nDownload:', 1)[0], '']
    return '\n'.join(lines).rstrip() + '\n'


def write_markdown(path):
    open(path, 'w', encoding='utf-8', newline='\n').write(markdown())


def _gh(*a, check=True, inp=None):
    r = subprocess.run(['gh', *a], capture_output=True, text=True, encoding='utf-8', errors='replace', input=inp)
    if check and r.returncode != 0:
        sys.exit('gh ' + ' '.join(a[:3]) + ' failed: ' + r.stderr.strip())
    return r


def sync_release(e, dry=False, latest=False):
    """Create or update the GitHub release of e's version, if its tag is on GitHub."""
    v = e['version']
    tag = 'v' + v
    body = release_body(e)
    if _gh('api', f'repos/{REPO}/git/ref/tags/{tag}', check=False).returncode != 0:
        print('   no tag', tag, 'on GitHub: skipped')
        return
    cur = _gh('release', 'view', tag, '-R', REPO, '--json', 'body', '-q', '.body', check=False)
    if cur.returncode == 0 and cur.stdout.strip() == body.strip():
        return
    if dry:
        print('   would', 'update' if cur.returncode == 0 else 'create', 'release', tag)
        return
    flags = ['--latest'] if latest else ['--latest=false']
    if cur.returncode == 0:
        _gh('release', 'edit', tag, '-R', REPO, '--title', 'Spool ' + v, '--notes-file', '-', *flags, inp=body)
        print('   release', tag, 'updated')
    else:
        _gh('release', 'create', tag, '-R', REPO, '--verify-tag', '--title', 'Spool ' + v, '--notes-file', '-', *flags, inp=body)
        print('   release', tag, 'created')


def sync_releases(dry=False, only=None):
    es = sorted(entries(), key=lambda e: _key(e['version']))
    newest = es[-1]['version']
    for e in es:
        if only and e['version'] != only:
            continue
        sync_release(e, dry=dry, latest=e['version'] == newest)


if __name__ == '__main__':
    if '--releases' in sys.argv:
        sync_releases(dry='--dry' in sys.argv)
    else:
        sys.stdout.reconfigure(encoding='utf-8')
        sys.stdout.write(markdown())
