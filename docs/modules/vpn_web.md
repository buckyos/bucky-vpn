# vpn_web

## 角色
VPN 项目的 Flutter Web 前端。

## 输入
- 后端 API 合同
- `vpn_web/lib/api.dart` 中的生成模型注解
- Flutter/Dart 构建与分析工具

## 输出
- Web UI
- 生成的 API 序列化代码
- Web 构建产物

## 依赖
- Flutter SDK
- Dart 工具链
- `vpn_web/pubspec.yaml` 中声明的依赖

## 边界
- 负责前端 UI 和客户端侧 API 接线
- 必须保持生成文件归工具所有，并确保模型字段与后端 key 不漂移
- 仓库默认规则：不要为 `vpn_web` 新增测试用例，除非用户明确要求例外
