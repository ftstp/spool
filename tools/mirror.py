# -*- coding: utf-8 -*-
"""The public repository (github.com/ftstp/spool): one commit per published version.

Development happens here, in a private history; the public repository gets a snapshot of
the code at each version, never the history (which carries working notes and an address).
Some paths stay private (EXCLUDE). publish.py --publish calls this after going online; it
can also be run alone:

    python tools/mirror.py            the snapshot of HEAD, committed and pushed
    python tools/mirror.py --dry      the snapshot only, nothing committed

The repository is PUBLIC since the 0.1.0 launch (2026-11-13): commits only go on top, never
a force-push. SPOOL_MIRROR_MESSAGE sets the commit message (default "Spool <version>").

The working copy of the public repository is ../spool-public (cloned on first use).
"""
import os, shutil, subprocess, sys, tarfile, io, json

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PUBLIC = os.path.join(os.path.dirname(ROOT), 'spool-public')
REMOTE = 'https://github.com/ftstp/spool.git'
AUTHOR = 'Fatstep <35991543+ftstp@users.noreply.github.com>'
EXCLUDE = ('docs/PLAN.md', 'docs/PARITY.md', 'tools/parity/')


def git(*a, cwd=PUBLIC, check=True):
    r = subprocess.run(['git', *a], cwd=cwd, capture_output=True, text=True, encoding='utf-8', errors='replace')
    if check and r.returncode != 0:
        print(r.stdout, r.stderr)
        sys.exit('git ' + ' '.join(a) + ' failed')
    return r.stdout.strip()


def main():
    dry = '--dry' in sys.argv
    v = json.load(open(os.path.join(ROOT, 'apps', 'desktop', 'tauri.conf.json'), encoding='utf-8'))['version']
    if git('status', '--porcelain', cwd=ROOT):
        sys.exit('commit the private repository first: the snapshot is HEAD')
    if not os.path.isdir(os.path.join(PUBLIC, '.git')):
        if subprocess.run(['git', 'clone', REMOTE, PUBLIC]).returncode != 0:
            os.makedirs(PUBLIC, exist_ok=True)
            git('init', '-b', 'main')
            git('remote', 'add', 'origin', REMOTE)
    # the working tree replaced by HEAD's files (the .git folder kept)
    for n in os.listdir(PUBLIC):
        if n != '.git':
            p = os.path.join(PUBLIC, n)
            shutil.rmtree(p) if os.path.isdir(p) else os.remove(p)
    tar = subprocess.run(['git', 'archive', '--format=tar', 'HEAD'], cwd=ROOT, capture_output=True).stdout
    with tarfile.open(fileobj=io.BytesIO(tar)) as t:
        members = [m for m in t.getmembers() if not m.name.startswith(EXCLUDE)]
        t.extractall(PUBLIC, members=members, filter='data')
    if dry:
        print('snapshot of', v, 'in', PUBLIC, '(nothing committed)')
        return
    git('add', '-A')
    if not git('status', '--porcelain'):
        print('nothing new for the public repository')
        return
    git('-c', 'user.name=Fatstep', '-c', 'user.email=35991543+ftstp@users.noreply.github.com', 'commit', '-q', '--author', AUTHOR, '-m', os.environ.get('SPOOL_MIRROR_MESSAGE') or ('Spool ' + v))
    git('tag', '-f', 'v' + v)
    git('push', '-u', 'origin', 'main')
    # the version's tag follows the snapshot (re-mirroring before a new version moves it)
    git('push', '-f', 'origin', 'v' + v)
    print('public repository: Spool', v)


if __name__ == '__main__':
    main()
