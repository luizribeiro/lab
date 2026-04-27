#!/usr/bin/env python3
import json
import sys


def main() -> int:
    request = json.load(sys.stdin)
    url = request["url"]
    response = {
        "schema_version": 1,
        "ok": True,
        "title": "Example Plugin Page",
        "url": url,
        "markdown": f"# Example\n\nFetched: {url}\n",
    }
    json.dump(response, sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())
