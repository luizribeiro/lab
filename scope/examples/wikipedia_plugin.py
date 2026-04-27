#!/usr/bin/env python3
import json
import sys
import urllib.parse
import urllib.request
from typing import Optional


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
    title = urllib.parse.unquote(raw_title)
    return lang, title


def fetch_extract(lang: str, title: str) -> tuple[str, str]:
    api = f"https://{lang}.wikipedia.org/w/api.php"
    params = urllib.parse.urlencode({
        "action": "query",
        "prop": "extracts",
        "explaintext": "1",
        "redirects": "1",
        "titles": title,
        "format": "json",
        "formatversion": "2",
    })
    req = urllib.request.Request(
        f"{api}?{params}",
        headers={"User-Agent": "scope-wikipedia-plugin/0.1 (https://github.com/luizribeiro/lab)"},
    )
    with urllib.request.urlopen(req, timeout=20) as resp:
        data = json.load(resp)
    pages = data.get("query", {}).get("pages", [])
    if not pages or "missing" in pages[0]:
        raise LookupError(f"page not found: {title}")
    page = pages[0]
    return page["title"], page.get("extract", "")


def render_markdown(title: str, extract: str, url: str) -> str:
    body = extract.strip() or "_(no extract available)_"
    return f"# {title}\n\n{body}\n\nSource: <{url}>\n"


def handle_request(request: dict) -> dict:
    url = request["url"]
    parsed = parse_wiki_url(url)
    if parsed is None:
        return {"schema_version": 1, "ok": False, "error": f"not a Wikipedia URL: {url}"}
    lang, title = parsed
    try:
        canonical_title, extract = fetch_extract(lang, title)
    except Exception as exc:
        return {"schema_version": 1, "ok": False, "error": str(exc)}
    return {
        "schema_version": 1,
        "ok": True,
        "title": canonical_title,
        "url": url,
        "markdown": render_markdown(canonical_title, extract, url),
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
        got = parse_wiki_url(url)
        assert got is None, f"{url}: expected None, got {got!r}"
    md = render_markdown("Berlin", "Berlin is the capital of Germany.",
                         "https://de.wikipedia.org/wiki/Berlin")
    assert md.startswith("# Berlin\n\n")
    assert "Berlin is the capital" in md
    assert "Source: <https://de.wikipedia.org/wiki/Berlin>" in md
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
