#!/usr/bin/env python3
# A fake github.com + api.github.com for the network tests of the install
# scripts (test-network.ps1, test-network.sh; install.yml; ADR-0096).
#
#   python3 fake-github.py <archive> <port-file>
#
# Listens on 127.0.0.1 on a free port and writes the port to <port-file>.
# The installers reach it through VOLT_INSTALL_TEST_SERVER =
# http://127.0.0.1:<port>/<scenario>-<run>; <run> is any word that keeps one
# test run's counters apart from the next. Paths below the scenario:
#
#   /api/repos/volt-hdl/volt/releases/latest        the API
#   /volt-hdl/volt/releases/latest                  the redirect
#   /volt-hdl/volt/releases/download/<tag>/<file>   release assets
#
# Scenarios:
#   none        no release: the API says 404, the redirect goes to
#               /releases, every asset is a 404
#   asset404    release v9.9.9 without the archive (SHA256SUMS is there)
#   cut         release v9.9.9; the archive download breaks off half way,
#               every time
#   ratelimit   the API answers 403 (rate limit); the redirect names v9.9.9
#   ratelimit-none  the API answers 429; the redirect goes to /releases
#   flaky       release v9.9.9; the API closes the connection without an
#               answer twice (.NET Framework silently repeats a request
#               once on a closed connection), the archive first breaks
#               off, then answers 503, then works
#   down        no HTTP answer at all, on every request
#
# A 404 is what github.com sends to a client without an Accept header: some
# 270 KB of chunked HTML. Windows PowerShell 5.1 reads at most 64 KB of an
# error response and then reports a closed connection instead of the 404.
import hashlib
import http.server
import json
import os
import sys
import threading

TAG = 'v9.9.9'
ARCHIVE = sys.argv[1]
ARCHIVE_NAME = os.path.basename(ARCHIVE)
with open(ARCHIVE, 'rb') as f:
    ARCHIVE_BYTES = f.read()
SUMS = ('%s  %s\n' % (hashlib.sha256(ARCHIVE_BYTES).hexdigest(), ARCHIVE_NAME)).encode()
NOT_FOUND_PAGE = b'<!DOCTYPE html><html><body>' + b'<p>Not Found</p>' * 17000 + b'</body></html>\n'

counts = {}
lock = threading.Lock()


def count(key):
    with lock:
        counts[key] = counts.get(key, 0) + 1
        return counts[key]


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, fmt, *args):
        sys.stderr.write('fake-github: %s %s\n' % (self.command, self.path))

    def do_GET(self):
        parts = self.path.split('?')[0].strip('/').split('/')
        scenario = parts[0].rsplit('-', 1)[0]
        rest = '/'.join(parts[1:])
        n = count(parts[0] + '/' + rest)
        handler = SCENARIOS.get(scenario)
        if handler is None:
            self.plain(400, b'unknown scenario\n')
            return
        handler(self, rest, n)

    # Answers ---------------------------------------------------------------
    def plain(self, status, body, ctype='text/plain; charset=utf-8', headers=()):
        self.send_response(status)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        for k, v in headers:
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def json(self, status, obj):
        self.plain(status, json.dumps(obj, indent=2).encode(), 'application/json; charset=utf-8')

    def not_found(self):
        self.send_response(404)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.send_header('Transfer-Encoding', 'chunked')
        self.end_headers()
        for i in range(0, len(NOT_FOUND_PAGE), 8192):
            chunk = NOT_FOUND_PAGE[i:i + 8192]
            self.wfile.write(b'%x\r\n%s\r\n' % (len(chunk), chunk))
        self.wfile.write(b'0\r\n\r\n')

    def redirect(self, location):
        self.plain(302, b'', 'text/html; charset=utf-8', [('Location', location)])

    def hang_up(self):
        self.close_connection = True
        self.connection.close()

    def cut(self, body):
        self.send_response(200)
        self.send_header('Content-Type', 'application/octet-stream')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body[:len(body) // 2])
        self.wfile.flush()
        self.hang_up()

    def server_base(self):
        return 'http://%s/%s/volt-hdl/volt' % (self.headers['Host'], self.path.strip('/').split('/')[0])

    # Building blocks -------------------------------------------------------
    def release(self, rest, archive=True):
        """A published release TAG: the API, the redirect and its assets."""
        if rest == 'api/repos/volt-hdl/volt/releases/latest':
            self.json(200, {'tag_name': TAG, 'name': 'Volt ' + TAG, 'draft': False, 'prerelease': False})
        elif rest == 'volt-hdl/volt/releases/latest':
            self.redirect(self.server_base() + '/releases/tag/' + TAG)
        elif rest == 'volt-hdl/volt/releases/download/%s/SHA256SUMS' % TAG:
            self.plain(200, SUMS, 'application/octet-stream')
        elif archive and rest == 'volt-hdl/volt/releases/download/%s/%s' % (TAG, ARCHIVE_NAME):
            self.plain(200, ARCHIVE_BYTES, 'application/octet-stream')
        else:
            self.not_found()

    def no_release(self, rest):
        if rest == 'api/repos/volt-hdl/volt/releases/latest':
            self.json(404, {'message': 'Not Found', 'status': '404'})
        elif rest == 'volt-hdl/volt/releases/latest':
            self.redirect(self.server_base() + '/releases')
        else:
            self.not_found()


def s_none(h, rest, n):
    h.no_release(rest)


def s_asset404(h, rest, n):
    h.release(rest, archive=False)


def s_cut(h, rest, n):
    if rest.endswith('/' + ARCHIVE_NAME):
        h.cut(ARCHIVE_BYTES)
    else:
        h.release(rest)


def s_ratelimit(h, rest, n):
    if rest.startswith('api/'):
        h.json(403, {'message': 'API rate limit exceeded for 127.0.0.1.'})
    else:
        h.release(rest)


def s_ratelimit_none(h, rest, n):
    if rest.startswith('api/'):
        h.json(429, {'message': 'Too many requests'})
    else:
        h.no_release(rest)


def s_flaky(h, rest, n):
    if rest.startswith('api/') and n <= 2:
        h.hang_up()
    elif rest.endswith('/' + ARCHIVE_NAME) and n == 1:
        h.cut(ARCHIVE_BYTES)
    elif rest.endswith('/' + ARCHIVE_NAME) and n == 2:
        h.plain(503, b'Service Unavailable\n')
    else:
        h.release(rest)


def s_down(h, rest, n):
    h.hang_up()


SCENARIOS = {
    'none': s_none,
    'asset404': s_asset404,
    'cut': s_cut,
    'ratelimit': s_ratelimit,
    'ratelimit-none': s_ratelimit_none,
    'flaky': s_flaky,
    'down': s_down,
}

if __name__ == '__main__':
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.daemon_threads = True
    tmp = sys.argv[2] + '.tmp'
    with open(tmp, 'w') as f:
        f.write(str(server.server_address[1]))
    os.replace(tmp, sys.argv[2])
    server.serve_forever()
