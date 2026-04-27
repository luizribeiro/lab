#!/usr/bin/env python3
import html
import json
import os
import re
import sys
import urllib.parse
import urllib.request
from typing import Optional

USER_AGENT = "scope-wikipedia-search-plugin/0.1 (https://github.com/luizribeiro/lab)"
DEFAULT_LANG = "en"
DEFAULT_LIMIT = 10
_LOCALE_RE = re.compile(r"^([a-z]{2,3})(?:[_-]|\.|$)")
_TAG_RE = re.compile(r"<[^>]+>")
_WS_RE = re.compile(r"\s+")


def lang_from_locale(value: Optional[str]) -> Optional[str]:
    if not value or value in ("C", "POSIX"):
        return None
    match = _LOCALE_RE.match(value)
    return match.group(1) if match else None


def detect_lang(env: dict) -> str:
    for key in ("LC_ALL", "LC_MESSAGES", "LANG"):
        lang = lang_from_locale(env.get(key))
        if lang:
            return lang
    return DEFAULT_LANG


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
    lang = detect_lang(os.environ)
    try:
        raw = fetch_search(lang, query, limit)
    except Exception as exc:
        return {"schema_version": 1, "ok": False, "error": str(exc)}
    return {
        "schema_version": 1,
        "ok": True,
        "results": to_results(lang, raw),
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

    assert lang_from_locale("en_US.UTF-8") == "en"
    assert lang_from_locale("fr_FR.UTF-8") == "fr"
    assert lang_from_locale("de_DE") == "de"
    assert lang_from_locale("pt-BR") == "pt"
    assert lang_from_locale("zh_CN.UTF-8") == "zh"
    assert lang_from_locale("ja") == "ja"
    assert lang_from_locale("C") is None
    assert lang_from_locale("POSIX") is None
    assert lang_from_locale("") is None
    assert lang_from_locale(None) is None
    assert lang_from_locale("garbage") is None

    assert detect_lang({"LC_ALL": "fr_FR.UTF-8", "LANG": "en_US.UTF-8"}) == "fr"
    assert detect_lang({"LANG": "de_DE.UTF-8"}) == "de"
    assert detect_lang({"LC_ALL": "C", "LANG": "ja_JP.UTF-8"}) == "ja"
    assert detect_lang({}) == "en"
    assert detect_lang({"LC_ALL": "C", "LANG": "POSIX"}) == "en"

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
