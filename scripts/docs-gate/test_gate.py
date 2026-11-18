#!/usr/bin/env python3
"""门禁回归测试（审计 D6/D7）：合成变更验证项目级跨模块规则可达 + 通配目录展开 + migrations/web 盲区闭合。
运行：python3 scripts/docs-gate/test_gate.py
"""
import unittest

import gate


class GateRegression(unittest.TestCase):
    def setUp(self):
        self.mods = gate.module_rules()
        self.proj = gate.project_rules()

    def _omission(self, chg):
        fails = []
        gate.check_omission(chg, self.mods, self.proj, False, fails)
        return fails

    def test_d6_kernel_api_cross_module_reachable(self):
        # kernel-api 契约变更须牵连 architecture + 逐模块 docs（审计 D6：项目规则可达）。
        fails = self._omission(["crates/kernel-api/src/lib.rs"])
        joined = "\n".join(fails)
        self.assertIn("docs/project/architecture.md", joined)
        self.assertIn("crates/kernel/docs/", joined)
        self.assertIn("crates/godoword/docs/", joined)

    def test_d6_kernel_api_satisfied_when_docs_updated(self):
        chg = ["crates/kernel-api/src/lib.rs", "docs/project/architecture.md"]
        # 逐模块 docs 全部更新 + history → 无漏改。
        for crate in self.mods:
            chg.append(f"crates/{crate}/docs/README.md")
            chg.append(f"crates/{crate}/wiki/README.md")
        chg.append("history/2026-01-01-0000-x.md")
        fails = self._omission(chg)
        self.assertEqual(fails, [], f"应无漏改：{fails}")

    def test_d6_wildcard_dir_prefix_match(self):
        # crates/*/docs/ 展开为逐模块前缀，普通文档文件变更可满足（审计 D6）。
        self.assertTrue(gate.target_satisfied("crates/kernel/docs/", {"crates/kernel/docs/README.md"}))
        self.assertFalse(gate.target_satisfied("crates/kernel/docs/", {"crates/kernel/src/lib.rs"}))
        expanded = gate.expand_target("crates/*/docs/")
        self.assertIn("crates/kernel/docs/", expanded)
        self.assertIn("crates/godoword/docs/", expanded)

    def test_d7_migration_only_triggers_omission_and_status(self):
        chg = ["crates/kernel/migrations/0009_probe.sql"]
        fails = self._omission(chg)
        self.assertTrue(any("crates/kernel/docs/" in f for f in fails), "migration 须触发 docs 漏改")
        sfails = []
        gate.check_status(chg, False, sfails)
        self.assertTrue(sfails, "migration-only 须触发 status 新鲜度")

    def test_d7_web_only_triggers_status(self):
        sfails = []
        gate.check_status(["web/shell/public/index.html"], False, sfails)
        self.assertTrue(sfails, "web-only 须触发 status 新鲜度")

    def test_d7_migration_coverage(self):
        fails = []
        gate.check_coverage(["crates/kernel/migrations/0009_probe.sql"], self.mods, self.proj, fails)
        self.assertEqual(fails, [], "kernel migrations 须命中 module.yaml 规则")


if __name__ == "__main__":
    unittest.main()