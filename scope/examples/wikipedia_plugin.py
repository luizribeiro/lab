#!/usr/bin/env python3
import json
import shutil
import subprocess
import sys
import urllib.parse
import urllib.request
from typing import Optional

USER_AGENT = "scope-wikipedia-plugin/0.1 (https://github.com/luizribeiro/lab)"


def parse_wiki_url(url: str) -> Optional[tuple[str, str]]:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme not in ("http", "https"):
        return None
    host = parsed.netloc.lower()
    if not host.endswith(".wikipedia.org"):
        return None
    lang = host.removesuffix(".wikipedia.org")
    if not lang or "." in lang:
        return None
    if not parsed.path.startswith("/wiki/"):
        return None
    raw_title = parsed.path[len("/wiki/"):]
    if not raw_title:
        return None
    return lang, urllib.parse.unquote(raw_title)


def fetch_html(lang: str, title: str) -> tuple[str, str]:
    encoded = urllib.parse.quote(title, safe="")
    url = f"https://{lang}.wikipedia.org/api/rest_v1/page/html/{encoded}"
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=20) as resp:
        body = resp.read().decode("utf-8")
        canonical = resp.headers.get("Content-Location", url)
    return canonical, body


def html_to_markdown(html: str) -> str:
    result = subprocess.run(
        ["pandoc", "--from=html", "--to=gfm-raw_html", "--wrap=none"],
        input=html,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def derive_title(canonical_url: str, fallback: str) -> str:
    parsed = urllib.parse.urlparse(canonical_url)
    if parsed.path.startswith("/api/rest_v1/page/html/"):
        slug = parsed.path[len("/api/rest_v1/page/html/"):]
        if slug:
            return urllib.parse.unquote(slug).replace("_", " ")
    return fallback.replace("_", " ")


def handle_request(request: dict) -> dict:
    url = request["url"]
    parsed = parse_wiki_url(url)
    if parsed is None:
        return {"schema_version": 1, "ok": False, "error": f"not a Wikipedia URL: {url}"}
    if shutil.which("pandoc") is None:
        return {"schema_version": 1, "ok": False, "error": "pandoc not found on PATH (required for HTML→markdown conversion)"}
    lang, title = parsed
    try:
        canonical_url, html = fetch_html(lang, title)
        markdown = html_to_markdown(html)
    except subprocess.CalledProcessError as exc:
        return {"schema_version": 1, "ok": False, "error": f"pandoc failed: {exc.stderr.strip() or exc}"}
    except Exception as exc:
        return {"schema_version": 1, "ok": False, "error": str(exc)}
    return {
        "schema_version": 1,
        "ok": True,
        "title": derive_title(canonical_url, title),
        "url": url,
        "markdown": markdown,
    }


def selftest() -> int:
    cases_ok = [
        ("https://en.wikipedia.org/wiki/Python_(programming_language)",
         ("en", "Python_(programming_language)")),
        ("https://fr.wikipedia.org/wiki/Pomme", ("fr", "Pomme")),
        ("https://en.wikipedia.org/wiki/Caf%C3%A9", ("en", "Café")),
        ("https://de.wikipedia.org/wiki/Berlin#Geschichte", ("de", "Berlin")),
    ]
    for url, expected in cases_ok:
        got = parse_wiki_url(url)
        assert got == expected, f"{url}: got {got!r}, want {expected!r}"
    cases_none = [
        "https://example.com/wiki/Foo",
        "ftp://en.wikipedia.org/wiki/Foo",
        "https://en.wikipedia.org/",
        "https://en.wikipedia.org/wiki/",
        "https://wikipedia.org/wiki/Foo",
    ]
    for url in cases_none:
        assert parse_wiki_url(url) is None, f"{url}: expected None"
    assert derive_title(
        "https://en.wikipedia.org/api/rest_v1/page/html/Berlin", "Berlin"
    ) == "Berlin"
    assert derive_title("https://x/", "Some_Page") == "Some Page"
    err = handle_request({"url": "https://example.com/wiki/Foo"})
    assert err == {"schema_version": 1, "ok": False, "error": "not a Wikipedia URL: https://example.com/wiki/Foo"}
    print("ok")
    return 0


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--selftest":
        return selftest()
    request = json.load(sys.stdin)
    response = handle_request(request)
    json.dump(response, sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())
