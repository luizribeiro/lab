#!/usr/bin/env python3
import html
import json
import re
import sys
import urllib.parse
import urllib.request
from typing import Optional

USER_AGENT = "scope-wikipedia-search-plugin/0.1 (https://github.com/luizribeiro/lab)"
LANG = "en"
DEFAULT_LIMIT = 10
_TAG_RE = re.compile(r"<[^>]+>")
_WS_RE = re.compile(r"\s+")


def strip_snippet_html(snippet: str) -> str:
    text = html.unescape(_TAG_RE.sub("", snippet))
    return _WS_RE.sub(" ", text).strip()


def article_url(lang: str, title: str) -> str:
    return f"https://{lang}.wikipedia.org/wiki/{urllib.parse.quote(title.replace(' ', '_'), safe=':()')}"


def fetch_search(lang: str, query: str, limit: int) -> list[dict]:
    api = f"https://{lang}.wikipedia.org/w/api.php"
    params = urllib.parse.urlencode({
        "action": "query",
        "list": "search",
        "srsearch": query,
        "srlimit": str(limit),
        "format": "json",
        "formatversion": "2",
    })
    req = urllib.request.Request(f"{api}?{params}", headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=20) as resp:
        data = json.load(resp)
    return data.get("query", {}).get("search", [])


def to_results(lang: str, raw: list[dict]) -> list[dict]:
    results = []
    for item in raw:
        title = item.get("title")
        if not title:
            continue
        snippet = strip_snippet_html(item.get("snippet", "")) or None
        results.append({
            "title": title,
            "url": article_url(lang, title),
            "snippet": snippet,
        })
    return results


def handle_request(request: dict) -> dict:
    query = request.get("query", "").strip()
    if not query:
        return {"schema_version": 1, "ok": False, "error": "empty query"}
    limit = request.get("limit") or DEFAULT_LIMIT
    try:
        raw = fetch_search(LANG, query, limit)
    except Exception as exc:
        return {"schema_version": 1, "ok": False, "error": str(exc)}
    return {
        "schema_version": 1,
        "ok": True,
        "results": to_results(LANG, raw),
    }


def selftest() -> int:
    assert strip_snippet_html(
        '<span class="searchmatch">Python</span> is a high-level language.'
    ) == "Python is a high-level language."
    assert strip_snippet_html("Caf&eacute; &amp; tea") == "Café & tea"
    assert strip_snippet_html("a   b\n\nc") == "a b c"
    assert strip_snippet_html("&quot;hi&quot; &#039;there&#039;") == "\"hi\" 'there'"

    assert article_url("en", "Python (programming language)") == \
        "https://en.wikipedia.org/wiki/Python_(programming_language)"
    assert article_url("fr", "Pomme") == "https://fr.wikipedia.org/wiki/Pomme"
    assert article_url("de", "Café") == "https://de.wikipedia.org/wiki/Caf%C3%A9"

    raw = [
        {"title": "Python (programming language)",
         "snippet": '<span class="searchmatch">Python</span> is a language.'},
        {"title": "Monty Python", "snippet": "British comedy group."},
        {"snippet": "missing title — skipped"},
        {"title": "No snippet"},
    ]
    results = to_results("en", raw)
    assert len(results) == 3, results
    assert results[0]["title"] == "Python (programming language)"
    assert results[0]["url"] == "https://en.wikipedia.org/wiki/Python_(programming_language)"
    assert results[0]["snippet"] == "Python is a language."
    assert results[2]["snippet"] is None

    assert handle_request({"query": "  "}) == {
        "schema_version": 1, "ok": False, "error": "empty query"
    }
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
