---
module: vpn_web
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-04-16
---

# vpn_web Design

> 本文件描述当前前端结构。验证细节放在 `testing.md`。

## 设计范围
### Goals
- 固化当前 Flutter Web 应用结构
- 明确路由、页面/对话框 UI 与 API 模型层的归属
- 保持生成文件边界清晰

### Non-goals
- 在本任务中重做 UI 或重构路由模型
- 在本任务中修改后端合同

## 总体方案
`vpn_web` 以 `main.dart` 为入口，使用 `GoRouter` 提供 `/` 和 `/login` 两个路由入口；页面和对话框位于 `lib/`；API 模型在 `api.dart` 中声明，序列化代码由 `api.g.dart` 生成。

## 模块拆分
| 子模块 | 类型 | 职责 | 输入 | 输出 | 依赖 | 是否单独文档 |
|--------|------|------|------|------|------|--------------|
| `main.dart` | assembly | 应用入口、主题与路由 | Flutter runtime | 路由后的应用壳 | Flutter, `go_router` | no |
| 页面 widgets | business | 首页、登录页、网络页等主要视图 | 路由状态和 API 数据 | 渲染后的 UI | Flutter widgets 和 API 层 | no |
| 对话框 widgets | business | 编辑、输入、提示等交互流程 | UI 动作与模型值 | 瞬时交互 UI | Flutter widgets | no |
| `http_client.dart` 与 `api.dart` | shared | 后端请求封装和类型化模型 | 后端合同 | 类型化前端 API 能力 | dio, json annotations | no |
| `api.g.dart` | generated | 序列化/反序列化生成代码 | `api.dart` 注解 | 模型胶水代码 | build_runner/json_serializable | no |

## 实现顺序
| 阶段 | 目标 | 前置条件 | 产出 | 依赖 | 可否并行 |
|------|------|----------|------|------|----------|
| 1 | 调整 API 模型或请求封装 | 已批准 proposal/design | 更新后的模型或 client 层 | none | yes |
| 2 | 调整路由、页面或对话框 | 若 API 面变化，则依赖阶段 1 | 更新后的 UI 流程 | 1 | yes |
| 3 | 重新生成生成文件并验证应用 | 代码改动完成 | analyze/test/build 证据 | 1,2 | no |

## 关键决策
- 路由继续集中在 `main.dart` 中维护
- `api.g.dart` 继续保持工具拥有，不允许手工维护
- 页面 widget 和对话框 widget 分离，保持交互流程可组合

## 数据与状态
- 路由级状态由 Flutter widget tree 和 router 管理
- API 模型定义在 `api.dart`，并由 `api.g.dart` 负责序列化实现
- 后端请求通过 `http_client.dart` 统一发出

## 接口与依赖
### 对外接口摘要
- Flutter 应用 `vpn_web`
- `/` 与 `/login` 两个主要路由入口
- `lib/` 下的 API wrapper 与数据模型

### 对外 HTTP 接口细节
- 前端只消费后端 HTTP API，不对外暴露 HTTP 接口

### 对外代码接口细节
- `vpn_web/lib/main.dart`
- `vpn_web/lib/` 下的页面/对话框/widget 模块
- `vpn_web/lib/api.g.dart` 作为生成边界

### 依赖接口与外部约束
- 修改 `api.dart` 模型或注解后，必须重新生成 `api.g.dart`
- 前端字段必须持续与后端 key 和 `(HttpResult, Data?)` 模式对齐

## 实现布局
```text
vpn_web/lib
├── main.dart
├── home.dart
├── login.dart
├── networks_page.dart
├── network_members_page.dart
├── joined_nodes_page.dart
├── dialogs...
├── http_client.dart
├── api.dart
└── api.g.dart
```

| 路径 | 类型 | 职责 | 备注 |
|------|------|------|------|
| `vpn_web/lib/main.dart` | file | 应用外壳、主题、路由 | 当前入口 |
| `vpn_web/lib/api.dart` | file | 注解模型与 API wrapper | 生成文件源头 |
| `vpn_web/lib/api.g.dart` | generated file | 序列化/反序列化生成代码 | 不要手工编辑 |
| `vpn_web/lib/*dialog*.dart` | files | 复用型对话框交互 | UI 流程助手 |

## 文档索引
| 文档 | 主题 | 范围 |
|------|------|------|
| `design.md` | 当前前端结构与边界 | full module |

## 风险与回滚
- 生成模型漂移是最主要的合同风险
- 当前 widget 测试与真实应用壳不一致，后续应作为单独问题治理
