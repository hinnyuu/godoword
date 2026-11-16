---
date: 2026-11-16T06:30+08:00
keywords: [assessment, plan, planprovider, placement, teacher-prompt]
scope: [project, teach-assess]
type: decision
status: accepted
supersedes: []
superseded_by: []
pr: ""
---

# 评估模块为教学核心

## 背景

用户主张 v1 立即引入评估模块：它决定其他教学模块的难度与侧重点，为 LLM 老师制订教学方案。

## 决策

teach-assess 实现 PlanProvider（薄接口 current_plan）。LearningPlan 双重表示：
structured（水平/侧重点/短板 JSON，供 UI 与难度逻辑）+ teacher_prompt（渲染好的系统提示词片段）
+ version + updated_at。教学模块会话系统提示词 = 模块 persona + plan.teacher_prompt + 模块约束。
会话启动钉住 plan version；resume 采用当前版本。冷启动：新用户 placement（LLM 短测或自评）
→ 初始方案；current_plan 返回 None → 前端引导 placement。方案数据 mod_teach_assess_* 私有，
他模块只经接口读。重规划并发锁。v1 教学模块 = teach-gojuon + teach-dialogue（均自适应）。

## 后果

评估内部实现（提取/重规划阈值）另开专轮。PlanProvider 保持薄，bootstrap/消费封装模块内。
见 [architecture](../docs/project/architecture.md)。