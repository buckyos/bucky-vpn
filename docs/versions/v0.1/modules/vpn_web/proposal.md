---
module: vpn_web
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-16
---

# vpn_web Proposal

## 背景与目标
`vpn_web` 是 Flutter Web 前端。本 packet 用于固化当前路由/UI 结构和验证面，使后续前端工作可以按 harness 工作流执行，而不是依赖隐含上下文。

## 范围
### In scope
- Flutter 应用根和路由
- `vpn_web/lib/` 下的页面与对话框
- `api.dart` 与生成文件 `api.g.dart` 组成的 API/数据模型层
- 当前前端验证入口

### Out of scope
- Rust 客户端与服务端行为
- 后端合同重设计
- 不经过工具链的生成文件改动

### Boundary with neighboring modules
- `vpn_web` 负责前端 UI 和 API wrapper 使用方式
- 生成代码与后端字段对齐必须保持稳定

## 约束
- 允许使用的库/组件：现有 Flutter/Dart 依赖、当前生成模型模式、当前页面/对话框结构
- 禁止采用的方案：手工修改 `api.g.dart` 这类生成文件、把后端职责迁入前端
- 系统约束：保持 `flutter analyze` 通过，保持显式字段类型和后端 key 对齐

## 高层结果
- 后续 Web UI 任务拥有已批准的模块 packet 和统一验证命令
- 路由、页面、对话框和 API 模型的归属清晰可见
- 已知测试面缺口被显式记录，而不是被假定不存在

## 风险
- API 模型改动可能导致生成代码与后端合同脱节
- 当前 `widget_test.dart` 仍是脚手架默认 counter 测试，与真实应用不一致
