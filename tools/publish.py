# -*- coding: utf-8 -*-
"""Publishes a version of Spool: installed copies see it and offer to update.

ONE COMMAND, in this order, each step refusing to go on if the one before failed:

    python tools/publish.py                    where things stand (nothing changes)
    python tools/publish.py --bump minor       0.1.0 -> 0.2.0 (patch | minor | major)
    python tools/publish.py --build            tests, then the signed installer, kept in releases/
    python tools/publish.py --publish          --build, then online, then checked from outside

--publish needs, in the environment (never in the repository): CLOUDFLARE_API_TOKEN,
CLOUDFLARE_ACCOUNT_ID and SPOOL_UPDATER_KEY_PASSWORD.

WHAT GOES ONLINE (artists.fatstep.com/spool/download/, a Worker with no script):
    Spool-<v>-setup.exe   every version ever published (an upload replaces all the files,
                          so the archive in releases/ is uploaded whole)
    latest.json           what installed copies read: version, notes, signature, url
    changelog.json        every version's notes, for the download page

THE SIGNATURE. Each installer is signed with the key at SPOOL_UPDATER_KEY (default
~/.claude/spool-updater.key; its password is SPOOL_UPDATER_KEY_PASSWORD). Installed copies hold the public key and refuse anything else.
Lose the key and installed copies can never update again: keep a copy of it, and of its
password, somewhere safe.
"""
import argparse, datetime, hashlib, json, os, re, shutil, subprocess, sys, time, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CONF = os.path.join(ROOT, 'apps', 'desktop', 'tauri.conf.json')
CARGO = os.path.join(ROOT, 'Cargo.toml')
PKG = os.path.join(ROOT, 'web', 'package.json')
CHANGELOG = os.path.join(ROOT, 'CHANGELOG.json')
RELEASES = os.path.join(ROOT, 'releases')            # every published installer (gitignored)
CF = os.path.join(ROOT, 'tools', 'cloudflare')
BUILD = os.path.join(CF, 'build')
SITE = 'https://artists.fatstep.com/spool/download/'
KEY = os.path.expanduser(os.environ.get('SPOOL_UPDATER_KEY', '~/.claude/spool-updater.key'))
MAX_FILE = 25 * 1024 * 1024                           # Workers static assets, per file


def say(*parts):
    # the Windows console is cp1252 and wrangler prints emoji
    print(' '.join(str(p) for p in parts).encode('ascii', 'replace').decode('ascii'), flush=True)


def stop(why):
    say('STOP:', why)
    sys.exit(1)


def run(cmd, cwd=ROOT, env=None, show=True):
    say('$', ' '.join(cmd))
    r = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, encoding='utf-8', errors='replace', shell=os.name == 'nt')
    if r.returncode != 0:
        say((r.stdout or '')[-3000:])
        say((r.stderr or '')[-3000:])
        stop('the command failed')
    if show:
        for line in (r.stdout or '').strip().splitlines()[-4:]:
            say('   ', line)
    return r.stdout


def version():
    return json.load(open(CONF, encoding='utf-8'))['version']


def set_version(v):
    """The same number in the three places that carry it."""
    for path, pattern, repl in [
        (CONF, r'"version": "[^"]+"', '"version": "%s"' % v),
        (CARGO, r'(\[workspace\.package\][^\[]*?version = )"[^"]+"', r'\g<1>"%s"' % v),
        (PKG, r'"version": "[^"]+"', '"version": "%s"' % v),
    ]:
        s = open(path, encoding='utf-8').read()
        n = re.sub(pattern, repl, s, count=1, flags=re.S)
        if n == s and ('"%s"' % v) not in s:
            stop('version not found in ' + path)
        open(path, 'w', encoding='utf-8', newline='').write(n)


def notes_of(v):
    for e in json.load(open(CHANGELOG, encoding='utf-8')):
        if e.get('version') == v:
            n = e.get('notes')
            if not isinstance(n, dict) or not n.get('en') or not n.get('fr'):
                break
            return e
    stop('no notes for %s in CHANGELOG.json, in English and French ({"en": [...], "fr": [...]})' % v)


def installer_dir(v):
    return os.path.join(RELEASES, v)


def build(v):
    notes_of(v)
    if not os.path.isfile(KEY):
        stop('signing key missing: ' + KEY)
    if not os.environ.get('SPOOL_UPDATER_KEY_PASSWORD'):
        stop('SPOOL_UPDATER_KEY_PASSWORD missing: set it in the environment first')
    say('--- 1. tests ---')
    run(['cargo', 'test', '--workspace', '--quiet'])
    say('--- 2. the signed installer, version', v, '---')
    env = dict(os.environ, TAURI_SIGNING_PRIVATE_KEY=KEY, TAURI_SIGNING_PRIVATE_KEY_PASSWORD=os.environ['SPOOL_UPDATER_KEY_PASSWORD'])
    tauri = os.path.join(ROOT, 'web', 'node_modules', '.bin', 'tauri' + ('.cmd' if os.name == 'nt' else ''))
    run([tauri, 'build', '--bundles', 'nsis'], cwd=os.path.join(ROOT, 'apps', 'desktop'), env=env)
    # the preview's copies of a real catalogue must never ship inside the app
    dist = os.path.join(ROOT, 'web', 'dist')
    leaked = [f for f in os.listdir(dist) if f.startswith('dev-') and f.endswith('.json')]
    if leaked:
        stop('preview data in the build (%s): the installer would carry them' % ', '.join(leaked))
    nsis = os.path.join(ROOT, 'target', 'release', 'bundle', 'nsis')
    exe = [f for f in os.listdir(nsis) if f.endswith('-setup.exe') and '_%s_' % v in f]
    if len(exe) != 1:
        stop('installer for %s not found in %s' % (v, nsis))
    src = os.path.join(nsis, exe[0])
    if not os.path.isfile(src + '.sig'):
        stop('the installer is not signed (no .sig): check the key and its password')
    if os.path.getsize(src) > MAX_FILE:
        stop('installer over 25 MiB: Cloudflare would refuse it')
    out = installer_dir(v)
    os.makedirs(out, exist_ok=True)
    dst = os.path.join(out, 'Spool-%s-setup.exe' % v)
    shutil.copyfile(src, dst)
    shutil.copyfile(src + '.sig', dst + '.sig')
    say('   kept:', dst, '(%.1f MB)' % (os.path.getsize(dst) / 1e6))
    return dst


def assemble(latest):
    """build/spool/download/: every published installer, latest.json, changelog.json."""
    shutil.rmtree(BUILD, ignore_errors=True)
    d = os.path.join(BUILD, 'spool', 'download')
    os.makedirs(d)
    shutil.copyfile(os.path.join(CF, '_headers'), os.path.join(BUILD, '_headers'))
    files = []
    for v in sorted(os.listdir(RELEASES)):
        exe = os.path.join(installer_dir(v), 'Spool-%s-setup.exe' % v)
        if os.path.isfile(exe):
            shutil.copyfile(exe, os.path.join(d, os.path.basename(exe)))
            files.append(os.path.basename(exe))
    # the tour shown on the download page (media/, rendered with Remotion), if it was made
    tour = os.path.join(ROOT, 'media', 'out', 'spool-tour.mp4')
    if os.path.isfile(tour):
        if os.path.getsize(tour) > MAX_FILE:
            stop('the tour video is over 25 MiB: Cloudflare would refuse it')
        shutil.copyfile(tour, os.path.join(d, 'spool-tour.mp4'))
        files.append('spool-tour.mp4')
    e = notes_of(latest)
    exe = 'Spool-%s-setup.exe' % latest
    sig = open(os.path.join(installer_dir(latest), exe + '.sig'), encoding='utf-8').read().strip()
    size = os.path.getsize(os.path.join(d, exe))
    manifest = {
        'version': latest,
        'notes': '\n'.join(e['notes']['en']),  # the updater shows one text: English
        'pub_date': datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace('+00:00', 'Z'),
        'platforms': {'windows-x86_64': {'signature': sig, 'url': SITE + exe}},
        # for the download page (the updater ignores them)
        'download': {'url': SITE + exe, 'size': size, 'sha256': hashlib.sha256(open(os.path.join(d, exe), 'rb').read()).hexdigest()},
    }
    json.dump(manifest, open(os.path.join(d, 'latest.json'), 'w', encoding='utf-8'), ensure_ascii=False, indent=2)
    log = [x for x in json.load(open(CHANGELOG, encoding='utf-8')) if x['version'] in [f[6:-10] for f in files]]
    json.dump(log, open(os.path.join(d, 'changelog.json'), 'w', encoding='utf-8'), ensure_ascii=False, indent=2)
    say('   online after this:', ', '.join(files), '+ latest.json + changelog.json')
    return d


def fetch(url):
    req = urllib.request.Request(url + '?t=%d' % time.time(), headers={'User-Agent': 'Spool-Publish/1.0 (check)', 'Accept-Encoding': 'identity'})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.status, r.read()


def publish(v):
    for k in ('CLOUDFLARE_API_TOKEN', 'CLOUDFLARE_ACCOUNT_ID'):
        if not os.environ.get(k):
            stop(k + ' missing: set it in the environment first')
    d = assemble(v)
    say('--- 3. online (wrangler deploy) ---')
    run(['npx', '--yes', 'wrangler@4', 'deploy', '--message', 'Spool ' + v], cwd=CF)
    say('--- 4. checked from outside, byte for byte ---')
    for name in ('latest.json', 'changelog.json', 'Spool-%s-setup.exe' % v):
        local = open(os.path.join(d, name), 'rb').read()
        for attempt in range(6):
            try:
                code, got = fetch(SITE + name)
                if code == 200 and hashlib.sha256(got).digest() == hashlib.sha256(local).digest():
                    say('   ok', name)
                    break
            except Exception as e:  # propagation: try again
                got = str(e).encode()
            time.sleep(4)
        else:
            stop('%s is not served as published (%s)' % (name, got[:120]))
    say('Spool %s is online. Installed copies will offer it within 6 hours (or at their next start).' % v)
    say('--- 5. the public repository ---')
    run([sys.executable, os.path.join(ROOT, 'tools', 'mirror.py')])


def main():
    a = argparse.ArgumentParser(description='Publish a version of Spool')
    a.add_argument('--bump', choices=['patch', 'minor', 'major'])
    a.add_argument('--build', action='store_true')
    a.add_argument('--publish', action='store_true')
    o = a.parse_args()
    v = version()
    if o.bump:
        x, y, z = (int(n) for n in v.split('.'))
        v = {'patch': f'{x}.{y}.{z + 1}', 'minor': f'{x}.{y + 1}.0', 'major': f'{x + 1}.0.0'}[o.bump]
        set_version(v)
        say('version ->', v, '(add its notes to CHANGELOG.json)')
    if o.build or o.publish:
        if not os.path.isfile(os.path.join(installer_dir(v), 'Spool-%s-setup.exe' % v)) or o.build:
            build(v)
    if o.publish:
        publish(v)
    if not (o.bump or o.build or o.publish):
        done = sorted(os.listdir(RELEASES)) if os.path.isdir(RELEASES) else []
        say('version in the code:', v)
        say('built and kept:', ', '.join(done) or 'none')
        try:
            code, got = fetch(SITE + 'latest.json')
            say('online:', json.loads(got)['version'])
        except Exception as e:
            say('online: nothing yet (%s)' % e)


if __name__ == '__main__':
    main()
