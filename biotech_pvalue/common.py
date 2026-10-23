"""Small shared helpers: polite cached HTTP for SEC and Yahoo."""
import threading
import time

import requests

import config


class RateLimiter:
    def __init__(self, per_second):
        self.min_gap = 1.0 / per_second
        self.lock = threading.Lock()
        self.last = 0.0

    def wait(self):
        with self.lock:
            now = time.monotonic()
            sleep_for = self.last + self.min_gap - now
            if sleep_for > 0:
                time.sleep(sleep_for)
            self.last = time.monotonic()


_sec_limiter = RateLimiter(config.SEC_MAX_RPS)
_sec_session = requests.Session()
_sec_session.headers.update({"User-Agent": config.SEC_USER_AGENT,
                             "Accept-Encoding": "gzip, deflate"})


def http_get(url, session, limiter=None, retries=5, ok_404=False):
    """GET with retries and exponential backoff. Returns bytes or None (404)."""
    for attempt in range(retries):
        if limiter:
            limiter.wait()
        try:
            r = session.get(url, timeout=60)
        except requests.RequestException:
            time.sleep(2 ** attempt)
            continue
        if r.status_code == 200:
            return r.content
        if r.status_code == 404 and ok_404:
            return None
        if r.status_code in (429, 500, 502, 503, 504, 403):
            time.sleep(2 ** attempt + 1)
            continue
        raise RuntimeError(f"HTTP {r.status_code} for {url}")
    raise RuntimeError(f"giving up on {url}")


def sec_get(url, ok_404=False):
    return http_get(url, _sec_session, _sec_limiter, ok_404=ok_404)


def cached(path, fetch):
    """Return bytes at path, downloading with fetch() the first time.

    A 404 is remembered with a '.missing' marker so reruns stay offline."""
    if path.exists():
        return path.read_bytes()
    missing = path.with_suffix(path.suffix + ".missing")
    if missing.exists():
        return None
    data = fetch()
    if data is None:
        missing.parent.mkdir(parents=True, exist_ok=True)
        missing.touch()
        return None
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_bytes(data)
    tmp.rename(path)
    return data
