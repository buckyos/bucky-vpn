# vpn_web 不新增测试规则

## 目标
- 固化 `vpn_web` 默认不新增测试用例的仓库级例外规则

## 适用范围
- `vpn_web/test/`
- `vpn_web` 下新增或修改 Flutter 测试文件的任务
- 触及 `vpn_web` 的 implementation 与 bugfix 任务

## 规则
- 默认不要为 `vpn_web` 新增测试用例，除非用户明确要求例外
- 不要把“顺手补前端测试”当作 UI 或 API 层改动的常规后续动作
- 前端验证优先使用：
  - `flutter analyze`
  - `flutter build web`
  - targeted manual verification

## 允许例外
- 用户明确要求新增或修改前端测试
- 仓库未来引入更强、且明确替代本规则的版本化规则

## Review 指引
- 如果 `vpn_web` 改动同时新增或扩展了测试用例，而任务里没有用户显式授权，应判定为不符合仓库规则
- 发现前端验证覆盖不足时，应明确指出受此规则约束，而不是默默补测试
