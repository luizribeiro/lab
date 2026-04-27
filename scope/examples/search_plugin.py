#!/usr/bin/env python3
import json
import sys


def main() -> int:
    request = json.load(sys.stdin)
    query = request["query"]
    limit = request.get("limit") or 2
    results = [
        {
            "title": f"Example for {query} #{i}",
            "url": f"https://example.com/{i}",
            "snippet": f"Example result {i} for {query}.",
        }
        for i in range(1, limit + 1)
    ]
    response = {
        "schema_version": 1,
        "ok": True,
        "results": results,
    }
    json.dump(response, sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())
