#!/usr/bin/env python3
"""Rebuilds README.zh-TW.md from tools/README.zh-TW.template.md.

The Chinese README shares its tables and transcripts with README.md so the numbers cannot
drift apart: {{TABLE:n}} is the n-th Markdown table of README.md, {{CODE:n}} its n-th fenced
block. {{CORNERS_ZH}} and {{BENCH_ZH}} are the Chinese versions of the two result sections,
kept in tools/README.zh-TW.sections.md. Run from the repository root.
"""
import re

en = open("README.md", encoding="utf-8").read()
tables, code, cur, in_code = [], [], [], False
for line in en.split("\n"):
    if line.startswith("```"):
        if in_code:
            code.append("\n".join(cur + [line]))
            cur, in_code = [], False
        else:
            cur, in_code = [line], True
        continue
    if in_code:
        cur.append(line)
    elif line.startswith("|"):
        cur.append(line)
    elif cur:
        tables.append("\n".join(cur))
        cur = []
sections = dict(re.findall(r"<!-- (\w+) -->\n(.*?)\n<!-- end -->", open("tools/README.zh-TW.sections.md", encoding="utf-8").read(), re.S))
out = open("tools/README.zh-TW.template.md", encoding="utf-8").read()
out = re.sub(r"\{\{TABLE:(\d+)\}\}", lambda m: tables[int(m.group(1)) - 1], out)
out = re.sub(r"\{\{CODE:(\d+)\}\}", lambda m: code[int(m.group(1)) - 1], out)
out = re.sub(r"\{\{(\w+)\}\}", lambda m: sections[m.group(1)], out)
open("README.zh-TW.md", "w", encoding="utf-8").write(out)
print("README.zh-TW.md: %d tables, %d code blocks available" % (len(tables), len(code)))
