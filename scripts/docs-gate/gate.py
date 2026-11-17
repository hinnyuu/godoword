#!/usr/bin/env python3
"""关系图谱门禁。规则见 docs/project/gates.md。

模式：
  --local        本地预检（覆盖率/漏改/链接/术语/中文/禁入路径），base 默认 main
  --ci           CI 模式，--base 指定 PR base sha，豁免读 GODOWORD_DOCS_NOCHANGE
  --index        重新生成 history/INDEX.md 与 INDEX.json
  --check-index  校验索引是否为最新（派生物 freshness）
"""
import argparse
import fnmatch
import json
import os
import re
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
PROJECT_PREFIXES = ("scripts/", ".github/workflows/", "docker/", "deploy/", "web/")
CJK = re.compile(r"[\u4e00-\u9fff]")
LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
MD_DIRS = ("docs/", "wiki/", "history/")


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True).stdout.strip()


def tracked():
    return [l for l in git("ls-files").splitlines() if l]


def changed(base):
    out = git("diff", "--name-only", f"{base}...HEAD")
    if not out:
        out = git("diff", "--name-only", base, "HEAD")
    return [l for l in out.splitlines() if l]


def load_yaml(p):
    return yaml.safe_load((ROOT / p).read_text(encoding="utf-8")) or {}


def module_rules():
    rules = {}
    for mf in sorted(ROOT.glob("crates/*/module.yaml")):
        crate = mf.parent.name
        data = yaml.safe_load(mf.read_text(encoding="utf-8")) or {}
        rules[crate] = data.get("impact", [])
    return rules


def project_rules():
    return load_yaml("docs/project/impact-map.yaml").get("rules", [])


def md_files():
    files = [Path("README.md")] if (ROOT / "README.md").exists() else []
    for d in MD_DIRS:
        files += sorted((ROOT / d).rglob("*.md"))
    return files


def check_coverage(files, mods, proj, fails):
    for crate, rules in mods.items():
        prefix = f"crates/{crate}/src/"
        for f in files:
            if f.startswith(prefix):
                rel = f[len(f"crates/{crate}/"):]
                if not any(fnmatch.fnmatch(rel, r["match"]) for r in rules):
                    fails.append(f"[覆盖率] {f} 未命中 {crate}/module.yaml 任何 impact 规则")
    for f in files:
        if f.startswith(PROJECT_PREFIXES):
            if not any(fnmatch.fnmatch(f, r["match"]) for r in proj):
                fails.append(f"[覆盖率] {f} 未命中项目 impact-map 任何规则")


def target_satisfied(target, changed_set):
    if "*" in target:
        return any(fnmatch.fnmatch(c, target) for c in changed_set)
    if target.endswith("/"):
        return any(c.startswith(target) for c in changed_set)
    return target in changed_set


def check_omission(chg, mods, proj, waiver, fails):
    changed_set = set(chg)
    required_targets = set()
    history_required = False
    for f in chg:
        matched = []
        if f.startswith(PROJECT_PREFIXES):
            matched += [r for r in proj if fnmatch.fnmatch(f, r["match"])]
        m = re.match(r"crates/([^/]+)/src/", f)
        if m and m.group(1) in mods:
            crate = m.group(1)
            rel = f[len(f"crates/{crate}/"):]
            matched += [r for r in mods[crate] if fnmatch.fnmatch(rel, r["match"])]
        for r in matched:
            for d in r.get("docs", []):
                required_targets.add(d)
            if r.get("history") == "required":
                history_required = True
    for t in sorted(required_targets):
        if not target_satisfied(t, changed_set):
            if waiver:
                print(f"[漏改·豁免] {t} 未更新（docs-nochange）")
            else:
                fails.append(f"[漏改] 命中文档 {t} 未在本 PR 更新（或加 docs-nochange 豁免）")
    if history_required:
        has_history = any(c.startswith("history/20") and c.endswith(".md") for c in chg)
        if not has_history:
            if waiver:
                print("[漏改·豁免] 需 history 条目但未新增（docs-nochange）")
            else:
                fails.append("[漏改] 改动要求 history 条目，但本 PR 未新增 history/20*.md")


def _slug(text):
    """GitHub 风格锚点近似：小写，保留字母数字/CJK/-/_，空白转 -，去其余标点。"""
    res = []
    for ch in text.lower().strip():
        if ch.isalnum() or ch in "-_":
            res.append(ch)
        elif ch.isspace():
            res.append("-")
    return "".join(res)


def check_links(fails):
    for md in md_files():
        text = (ROOT / md).read_text(encoding="utf-8")
        for link in LINK.findall(text):
            if link.startswith(("http://", "https://", "mailto:")):
                continue
            path, _, anchor = link.partition("#")
            path = path.strip()
            if path:
                target = (md.parent / path).resolve()
                if not target.exists():
                    fails.append(f"[链接] {md} 断链 -> {link}")
                    continue
            else:
                target = (ROOT / md).resolve()
            if anchor and target.is_file() and target.suffix == ".md":
                headings = [
                    _slug(l.lstrip("#").strip())
                    for l in target.read_text(encoding="utf-8").splitlines()
                    if l.startswith("#")
                ]
                if _slug(anchor) not in headings:
                    fails.append(f"[链接] {md} 锚点失效 -> {link}")


def check_glossary(fails):
    terms = load_yaml("docs/project/glossary.yaml").get("terms", [])
    banned = [(t["canonical"], b) for t in terms for b in t.get("banned", [])]
    for md in md_files():
        # 历史正文不可变（append-only），术语检查豁免；写入时用词合规由当时门禁保证
        if "history" in md.parts:
            continue
        for i, line in enumerate((ROOT / md).read_text(encoding="utf-8").splitlines(), 1):
            for canonical, b in banned:
                if b in line:
                    fails.append(f"[术语] {md}:{i} 禁用别名 '{b}'（规范词：{canonical}）")


def check_chinese(fails):
    for md in md_files():
        if not CJK.search((ROOT / md).read_text(encoding="utf-8")):
            fails.append(f"[中文] {md} 解释型文档须含中文")


def check_forbidden(files, fails):
    for f in files:
        name = os.path.basename(f)
        if name == "devenv.md" or name.endswith(".key") or (
            name.startswith(".env") and name != ".env.example"
        ):
            fails.append(f"[禁入] {f} 严禁提交")


def build_index():
    entries = []
    for f in sorted((ROOT / "history").glob("20*.md")):
        txt = f.read_text(encoding="utf-8")
        m = re.match(r"^---\n(.*?)\n---\n(.*)$", txt, re.S)
        fm = yaml.safe_load(m.group(1))
        title = next((l[2:].strip() for l in m.group(2).splitlines() if l.startswith("# ")), f.name)
        entries.append({"file": f.name, "title": title, **fm})
    entries.sort(key=lambda e: e["date"])
    js = json.dumps(entries, ensure_ascii=False, indent=2)
    lines = ["# 历史索引", "", "> 由 scripts/docs-gate 从 frontmatter 生成（派生物，勿手改）。规则见 conventions.md。",
             "", f"共 {len(entries)} 条。", ""]
    for e in entries:
        kw = ", ".join(e.get("keywords", []))
        sc = ", ".join(e.get("scope", []))
        lines.append(f"- [{e['title']}]({e['file']}) — {e['date']} · {e['type']} · {e['status']} · scope: {sc} · keywords: {kw}")
    return "\n".join(lines) + "\n", js


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--local", action="store_true")
    ap.add_argument("--ci", action="store_true")
    ap.add_argument("--base", default="main")
    ap.add_argument("--index", action="store_true")
    ap.add_argument("--check-index", action="store_true")
    args = ap.parse_args()

    if args.index:
        md, js = build_index()
        (ROOT / "history/INDEX.md").write_text(md, encoding="utf-8")
        (ROOT / "history/INDEX.json").write_text(js, encoding="utf-8")
        print("索引已生成")
        return 0
    if args.check_index:
        md, js = build_index()
        ok = True
        if (ROOT / "history/INDEX.md").read_text(encoding="utf-8") != md:
            print("[索引] INDEX.md 过期，运行 gate.py --index"); ok = False
        if (ROOT / "history/INDEX.json").read_text(encoding="utf-8") != js:
            print("[索引] INDEX.json 过期，运行 gate.py --index"); ok = False
        return 0 if ok else 1

    files = tracked()
    mods = module_rules()
    proj = project_rules()
    waiver = os.environ.get("GODOWORD_DOCS_NOCHANGE", "").lower() in ("1", "true", "yes")
    fails = []
    check_coverage(files, mods, proj, fails)
    check_links(fails)
    check_glossary(fails)
    check_chinese(fails)
    check_forbidden(files, fails)
    if args.local or args.ci:
        check_omission(changed(args.base), mods, proj, waiver, fails)
    if fails:
        print("\n".join(fails))
        print(f"\n门禁失败：{len(fails)} 项")
        return 1
    print("门禁通过（覆盖率/漏改/链接/术语/中文/禁入路径）")
    return 0


if __name__ == "__main__":
    sys.exit(main())