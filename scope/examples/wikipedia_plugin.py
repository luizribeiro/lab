#!/usr/bin/env python3
import json
import shutil
import subprocess
import sys
import urllib.parse
import urllib.request
from html.parser import HTMLParser
from typing import Optional

USER_AGENT = "scope-wikipedia-plugin/0.1 (https://github.com/luizribeiro/lab)"
STRIP_TABLE_CLASS_PREFIXES = ("infobox", "navbox", "sidebar", "metadata", "ambox")


def _is_chrome_class(class_attr: str) -> bool:
    for token in class_attr.split():
        for prefix in STRIP_TABLE_CLASS_PREFIXES:
            if token == prefix or token.startswith(prefix + "-"):
                return True
    return False


class _ChromeTableStripper(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=False)
        self._out: list[str] = []
        self._table_stripping: list[bool] = []

    def _stripping(self) -> bool:
        return any(self._table_stripping)

    def handle_starttag(self, tag: str, attrs: list[tuple[str, Optional[str]]]) -> None:
        if tag == "table":
            self._table_stripping.append(_is_chrome_class(dict(attrs).get("class") or ""))
            if self._stripping():
                return
        if self._stripping():
            return
        self._out.append(_render_starttag(tag, attrs))

    def handle_endtag(self, tag: str) -> None:
        if tag == "table" and self._table_stripping:
            was_stripping = self._stripping()
            self._table_stripping.pop()
            if was_stripping:
                return
        if self._stripping():
            return
        self._out.append(f"</{tag}>")

    def handle_startendtag(self, tag: str, attrs: list[tuple[str, Optional[str]]]) -> None:
        if self._stripping():
            return
        self._out.append(_render_starttag(tag, attrs, self_close=True))

    def handle_data(self, data: str) -> None:
        if not self._stripping():
            self._out.append(data)

    def handle_entityref(self, name: str) -> None:
        if not self._stripping():
            self._out.append(f"&{name};")

    def handle_charref(self, name: str) -> None:
        if not self._stripping():
            self._out.append(f"&#{name};")

    def handle_comment(self, data: str) -> None:
        if not self._stripping():
            self._out.append(f"<!--{data}-->")

    def result(self) -> str:
        return "".join(self._out)


def _render_starttag(tag: str, attrs: list[tuple[str, Optional[str]]], self_close: bool = False) -> str:
    parts = [tag]
    for k, v in attrs:
        if v is None:
            parts.append(k)
        else:
            parts.append(f'{k}="{v.replace(chr(34), "&quot;")}"')
    suffix = " /" if self_close else ""
    return f"<{' '.join(parts)}{suffix}>"


def strip_chrome_tables(html: str) -> str:
    parser = _ChromeTableStripper()
    parser.feed(html)
    parser.close()
    return parser.result()


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
        [
            "pandoc",
            "--from=html",
            "--to=markdown_strict+pipe_tables+grid_tables+backtick_code_blocks+strikeout+autolink_bare_uris-raw_html",
            "--wrap=none",
        ],
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
        markdown = html_to_markdown(strip_chrome_tables(html))
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

    infobox = '<p>before</p><table class="infobox vcard"><tr><td>chrome</td></tr></table><p>after</p>'
    out = strip_chrome_tables(infobox)
    assert "chrome" not in out and "before" in out and "after" in out, out
    data_table = '<table class="wikitable"><tr><td>keep me</td></tr></table>'
    assert "keep me" in strip_chrome_tables(data_table)
    plain_table = "<table><tr><td>plain</td></tr></table>"
    assert "plain" in strip_chrome_tables(plain_table)
    nested = (
        '<table class="navbox"><tr><td>'
        '<table class="wikitable"><tr><td>inner</td></tr></table>'
        "</td></tr></table>"
    )
    assert "inner" not in strip_chrome_tables(nested)
    multiclass = '<table class="metadata plainlinks"><tr><td>x</td></tr></table>'
    assert "x" not in strip_chrome_tables(multiclass)
    not_chrome = '<table class="not-an-infobox"><tr><td>kept</td></tr></table>'
    assert "kept" in strip_chrome_tables(not_chrome)
    variants = '<table class="nowraplinks navbox-subgroup"><tr><td>nv</td></tr></table>'
    assert "nv" not in strip_chrome_tables(variants), "navbox-subgroup should be chrome"
    inner = '<table class="navbox-inner"><tr><td>ni</td></tr></table>'
    assert "ni" not in strip_chrome_tables(inner)

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
