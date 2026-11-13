# -*- coding: utf-8 -*-
"""Tests the update path end to end, on a throwaway copy of Spool that cannot touch the real one.

What it does:
  1. builds two installers of "Spool Update Test" (another name, another identifier, so another
     install folder and another data folder), versions 0.1.0 and 0.1.1, signed with Spool's
     key, asking http://127.0.0.1:8765/latest.json for updates; in their own target folder,
     so target/release/Spool.exe is not touched;
  2. serves 0.1.1 from a local server, installs 0.1.0 silently, starts it with
     SPOOL_UPDATE_SELFTEST=1 (it installs the update without a click and logs what happened);
     checks the installed program became 0.1.1;
  3. offers a "0.1.2" whose installer was altered by one byte (signature of the real one):
     the update must be REFUSED and the program stay 0.1.1;
  4. uninstalls the test program and deletes its data folder.

    set -a; . ~/.claude/fssf-secrets.env; set +a
    python tools/update_selftest.py
"""
import http.server, json, os, shutil, socketserver, subprocess, sys, threading, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORK = os.path.join(ROOT, 'target-selftest', 'work')
SERVE = os.path.join(WORK, 'serve')
NAME = 'Spool Update Test'
IDENT = 'app.spool.updatetest'
EXE = 'SpoolUpdateTest'
PORT = 8765
LOCAL = os.environ['LOCALAPPDATA']
INSTALL_DIR = os.path.join(LOCAL, NAME)
DATA_DIR = os.path.join(os.environ['APPDATA'], IDENT)
KEY = os.path.expanduser(os.environ.get('SPOOL_UPDATER_KEY', '~/.claude/spool-updater.key'))
TAURI = os.path.join(ROOT, 'web', 'node_modules', '.bin', 'tauri.cmd')
hits = []


def say(*a):
    print(' '.join(str(x) for x in a).encode('ascii', 'replace').decode('ascii'), flush=True)


def build(version):
    conf = {
        'identifier': IDENT, 'productName': NAME, 'mainBinaryName': EXE, 'version': version,
        'app': {'windows': [{'title': NAME, 'width': 900, 'height': 600}]},
        'plugins': {'updater': {'endpoints': [f'http://127.0.0.1:{PORT}/latest.json'], 'dangerousInsecureTransportProtocol': True}},
    }
    path = os.path.join(WORK, f'conf-{version}.json')
    json.dump(conf, open(path, 'w'))
    env = dict(os.environ, CARGO_TARGET_DIR=os.path.join(ROOT, 'target-selftest'), TAURI_SIGNING_PRIVATE_KEY=KEY,
               TAURI_SIGNING_PRIVATE_KEY_PASSWORD=os.environ['SPOOL_UPDATER_KEY_PASSWORD'])
    say('build', version)
    r = subprocess.run([TAURI, 'build', '--bundles', 'nsis', '--config', path], cwd=os.path.join(ROOT, 'apps', 'desktop'),
                       env=env, capture_output=True, text=True, encoding='utf-8', errors='replace', shell=True)
    if r.returncode:
        say(r.stdout[-2000:], r.stderr[-3000:])
        sys.exit('build failed')
    d = os.path.join(ROOT, 'target-selftest', 'release', 'bundle', 'nsis')
    exe = os.path.join(d, f'{NAME}_{version}_x64-setup.exe')
    out = os.path.join(WORK, f'setup-{version}.exe')
    shutil.copyfile(exe, out)
    shutil.copyfile(exe + '.sig', out + '.sig')
    return out


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=SERVE, **k)

    def log_message(self, fmt, *args):
        hits.append(self.path)


def serve_version(version, installer, sig_from=None):
    os.makedirs(SERVE, exist_ok=True)
    name = f'setup-{version}.exe'
    shutil.copyfile(installer, os.path.join(SERVE, name))
    sig = open((sig_from or installer) + '.sig', encoding='utf-8').read().strip()
    json.dump({'version': version, 'notes': 'self-test', 'pub_date': '2026-11-13T12:00:00Z',
               'platforms': {'windows-x86_64': {'signature': sig, 'url': f'http://127.0.0.1:{PORT}/{name}'}}},
              open(os.path.join(SERVE, 'latest.json'), 'w'))


def installed_version():
    exe = os.path.join(INSTALL_DIR, EXE + '.exe')
    if not os.path.isfile(exe):
        return None
    r = subprocess.run(['powershell', '-NoProfile', '-Command', f"(Get-Item '{exe}').VersionInfo.ProductVersion"], capture_output=True, text=True)
    return r.stdout.strip()


def stop_test_app():
    subprocess.run(['powershell', '-NoProfile', '-Command', f"Get-Process {EXE} -ErrorAction SilentlyContinue | Stop-Process -Force"], capture_output=True)


def run_selftest(wait=90):
    log = os.path.join(DATA_DIR, 'update-selftest.log')
    before = open(log, encoding='utf-8').read() if os.path.isfile(log) else ''
    env = dict(os.environ, SPOOL_UPDATE_SELFTEST='1')
    subprocess.Popen([os.path.join(INSTALL_DIR, EXE + '.exe')], env=env)
    t0 = time.time()
    while time.time() - t0 < wait:
        time.sleep(3)
        now = open(log, encoding='utf-8').read() if os.path.isfile(log) else ''
        new = now[len(before):]
        if 'refused' in new or 'installed' in new:
            time.sleep(25 if 'installed' in new else 2)  # let the installer finish
            return new
    return (open(log, encoding='utf-8').read() if os.path.isfile(log) else '')[len(before):] or 'nothing logged'


def main():
    if os.path.isdir(INSTALL_DIR) or os.path.isdir(DATA_DIR):
        sys.exit(f'a previous test is still installed: {INSTALL_DIR} / {DATA_DIR}')
    os.makedirs(WORK, exist_ok=True)
    new = build('0.1.1')
    old = build('0.1.0')
    srv = socketserver.TCPServer(('127.0.0.1', PORT), Handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    ok = True
    try:
        serve_version('0.1.1', new)
        say('install 0.1.0')
        subprocess.run([old, '/S'], check=True)
        time.sleep(3)
        say('installed:', installed_version())
        say('--- 1. a signed update ---')
        say(run_selftest().strip())
        stop_test_app()
        v = installed_version()
        say('installed now:', v, '| server saw:', hits)
        if not (v or '').startswith('0.1.1'):
            ok = False
            say('FAIL: the update did not install')
        say('--- 2. an altered installer ---')
        bad = os.path.join(WORK, 'setup-0.1.2-altered.exe')
        b = bytearray(open(new, 'rb').read())
        b[len(b) // 2] ^= 0xFF
        open(bad, 'wb').write(b)
        serve_version('0.1.2', bad, sig_from=new)
        hits.clear()
        out = run_selftest(60).strip()
        say(out)
        stop_test_app()
        v2 = installed_version()
        say('installed now:', v2, '| server saw:', hits)
        if 'refused' not in out or not (v2 or '').startswith('0.1.1'):
            ok = False
            say('FAIL: an altered installer was not refused')
    finally:
        srv.shutdown()
        stop_test_app()
        say('--- cleanup ---')
        unins = os.path.join(INSTALL_DIR, 'uninstall.exe')
        if os.path.isfile(unins):
            subprocess.run([unins, '/S'])
            time.sleep(4)
        shutil.rmtree(INSTALL_DIR, ignore_errors=True)
        shutil.rmtree(DATA_DIR, ignore_errors=True)
        say('test program removed:', not os.path.exists(INSTALL_DIR), '| its data removed:', not os.path.exists(DATA_DIR))
    say('RESULT:', 'PASS' if ok else 'FAIL')
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
