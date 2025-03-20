import 'dart:convert';
import 'dart:typed_data';
import 'dart:html';

import 'package:crypto/crypto.dart';
import 'package:flutter/foundation.dart';
import 'package:json_annotation/json_annotation.dart';

import 'base58.dart';
import 'http_client.dart';

part 'api.g.dart';

@JsonSerializable()
class LoginReq {
  @JsonKey(name: 'user_name')
  String userName;
  String password;
  int timestamp;

  LoginReq({required this.userName, required this.password, required this.timestamp});

  factory LoginReq.fromJson(Map<String, dynamic> json) => _$LoginReqFromJson(json);
  Map<String, dynamic> toJson() => _$LoginReqToJson(this);
}

@JsonSerializable()
class UserInfo {
  String id;
  @JsonKey(name: 'network_id')
  String networkId;
  @JsonKey(name: 'session_id')
  String sessionId;
  @JsonKey(name: 'server_id')
  String serverId;

  UserInfo({required this.id, required this.networkId, required this.sessionId, required this.serverId});

  factory UserInfo.fromJson(Map<String, dynamic> json) => _$UserInfoFromJson(json);
  Map<String, dynamic> toJson() => _$UserInfoToJson(this);
}

@JsonSerializable()
class JoinedNode {
  @JsonKey(name: 'group_id')
  String groupId;
  @JsonKey(name: 'node_id')
  String nodeId;
  @JsonKey(name: 'allow_join')
  bool allowJoin;
  String name;
  String comment;
  @JsonKey(name: 'online')
  bool isOnline;
  @JsonKey(name: 'client_version')
  String? clientVersion;
  @JsonKey(name: 'ip_list')
  List<String>? ipList;

  JoinedNode(
      {required this.groupId,
      required this.nodeId,
      required this.allowJoin,
      required this.name,
      required this.comment,
      required this.isOnline,
      this.clientVersion,
      this.ipList});

  factory JoinedNode.fromJson(Map<String, dynamic> json) => _$JoinedNodeFromJson(json);
  Map<String, dynamic> toJson() => _$JoinedNodeToJson(this);
}

@JsonSerializable()
class Network {
  String id;
  @JsonKey(name: 'group_id')
  String groupId;
  String name;
  @JsonKey(name: 'ip_seg')
  String? ipSeg;
  int mask;
  @JsonKey(name: 'ipv6_seg')
  String? ipv6Seg;
  @JsonKey(name: 'ipv6_mask')
  int ipv6Mask;

  Network({required this.id,
    required this.groupId,
    required this.name,
    this.ipSeg,
    required this.mask,
    this.ipv6Seg,
    required this.ipv6Mask});

  factory Network.fromJson(Map<String, dynamic> json) => _$NetworkFromJson(json);
  Map<String, dynamic> toJson() => _$NetworkToJson(this);
}

@JsonSerializable()
class NetworkMember {
  @JsonKey(name: 'id')
  String nodeId;
  @JsonKey(name: 'ip')
  String ipAddr;
  @JsonKey(name: 'online')
  bool isOnline;
  @JsonKey(name: 'client_version')
  String? clientVersion;
  @JsonKey(name: 'ip_list')
  List<String>? ipList;

  NetworkMember(
      {required this.nodeId,
        required this.ipAddr,
        required this.isOnline,
        this.clientVersion,
        this.ipList});

  factory NetworkMember.fromJson(Map<String, dynamic> json) =>
      _$NetworkMemberFromJson(json);

  Map<String, dynamic> toJson() => _$NetworkMemberToJson(this);
}

String calculateSha256(String input) {
  final bytes = utf8.encode(input); // 将字符串转换为字节
  final digest = sha256.convert(bytes); // 计算哈希
  return base58.encode(Uint8List.fromList(digest.bytes)); // 返回哈希值的字符串表示
}

class Api {
  HttpClient _client;
  static Api? _instance;

  Api._internal(String baseUrl): _client = HttpClient(baseUrl);

  static Api instance() {
    if (_instance == null) {
      if (kReleaseMode) {
        final host = Uri.base.host;
        final port = Uri.base.port;
        _instance = Api._internal("http://$host:$port/api");
      } else {
        _instance = Api._internal("http://192.168.80.100:3445");
      }
    }
    return _instance!;
  }

  Future<HttpResult> login(String userName, String password) async {
    String hashedPassword = calculateSha256("$userName$password");
    var timestamp = DateTime.now().millisecondsSinceEpoch;
    String saltPassword = calculateSha256("$hashedPassword$timestamp");
    final req = LoginReq(userName: userName, password: saltPassword, timestamp: timestamp);
    var (result, resp) = await _client.postJson("/account/login", req.toJson());
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          var data = resp["result"] as Map;
          window.localStorage["session"] = data["session"];
          window.localStorage["refresh_session"] = data["refresh_session"];
          return HttpResult(0);
        } else {
          return HttpResult(resp["err"], msg: resp["msg"] as String);
        }
      } else {
        return HttpResult(-1);
      }
    } else {
      return result;
    }
  }

  Future<(HttpResult, UserInfo?)> getUserInfo() async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return (HttpResult(-1), null);
    }

    var (result, resp) = await _client.getJson("/account/get_account_info", headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          return (HttpResult(0), UserInfo.fromJson(resp["result"]));
        } else {
          return (HttpResult(resp["err"], msg: resp["msg"] as String), null);
        }
      } else {
        return (HttpResult(-1), null);
      }
    } else {
      return (result, null);
    }
  }

  Future<(HttpResult, List<JoinedNode>?)> getJoinedNodes() async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return (HttpResult(-1), null);
    }

    var (result, resp) = await _client.getJson("/get_joined_nodes", headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          var data = resp["result"] as List;
          return (HttpResult(0), data.map((e) => JoinedNode.fromJson(e as Map<String, dynamic>)).toList());
        } else {
          return (HttpResult(resp["err"], msg: resp["msg"] as String), null);
        }
      } else {
        return (HttpResult(-1), null);
      }
    } else {
      return (result, null);
    }
  }

  Future<(HttpResult, List<Network>?)> getNetworks() async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return (HttpResult(-1), null);
    }

    var (result, resp) = await _client.getJson("/get_networks", headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          var data = resp["result"] as List;
          return (HttpResult(0), data.map((e) => Network.fromJson(e as Map<String, dynamic>)).toList());
        } else {
          return (HttpResult(resp["err"], msg: resp["msg"] as String), null);
        }
      } else {
        return (HttpResult(-1), null);
      }
    } else {
      return (result, null);
    }
  }

  Future<HttpResult> addNetwork(String name, String ipSeg, int mask) async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return HttpResult(-1);
    }

    var (result, resp) = await _client.postJson("/add_network", {"name": name, "ip_addr": ipSeg, "mask": mask}, headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          return HttpResult(0);
        } else {
          return HttpResult(resp["err"], msg: resp["msg"] as String);
        }
      } else {
        return HttpResult(-1);
      }
    } else {
      return result;
    }
  }

  Future<HttpResult> addNetworkMember(String networkId, String nodeId, String ipAddr) async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return HttpResult(-1);
    }

    var (result, resp) = await _client.postJson("/add_network_member", {"network_id": networkId, "node_id": nodeId, "ip_addr": ipAddr}, headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          return HttpResult(0);
        } else {
          return HttpResult(resp["err"], msg: resp["msg"] as String);
        }
      } else {
        return HttpResult(-1);
      }
    } else {
      return result;
    }
  }

  Future<HttpResult> updateNetworkMember(String networkId, String nodeId, String ipAddr) async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return HttpResult(-1);
    }

    var (result, resp) = await _client.postJson("/update_network_member", {"network_id": networkId, "node_id": nodeId, "ip_addr": ipAddr}, headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          return HttpResult(0);
        } else {
          return HttpResult(resp["err"], msg: resp["msg"] as String);
        }
      } else {
        return HttpResult(-1);
      }
    } else {
      return result;
    }
  }

  Future<HttpResult> deleteNetworkMember(String networkId, String nodeId) async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return HttpResult(-1);
    }

    var (result, resp) = await _client.postJson("/delete_network_member", {"network_id": networkId, "node_id": nodeId}, headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          return HttpResult(0);
        } else {
          return HttpResult(resp["err"], msg: resp["msg"] as String);
        }
      } else {
        return HttpResult(-1);
      }
    } else {
      return result;
    }
  }

  Future<(HttpResult, List<NetworkMember>?)> getNetworkMember(String networkId) async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return (HttpResult(-1), null);
    }

    var (result, resp) = await _client.postJson("/get_network_member", {"network_id": networkId}, headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          var data = resp["result"] as List;
          return (HttpResult(0), data.map((e) => NetworkMember.fromJson(e as Map<String, dynamic>)).toList());
        } else {
          return (HttpResult(resp["err"], msg: resp["msg"] as String), null);
        }
      } else {
        return (HttpResult(-1), null);
      }
    } else {
      return (result, null);
    }
  }

  Future<HttpResult> allowJoin(String nodeId, bool allowJoin) async {
    String? session = window.localStorage["session"];
    if (session == null) {
      return HttpResult(-1);
    }

    var (result, resp) = await _client.postJson("/allow_join", {"node_id": nodeId, "allow_join": allowJoin}, headers: {"authorization": "Bearer $session"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          return HttpResult(0);
        } else {
          return HttpResult(resp["err"], msg: resp["msg"] as String);
        }
      } else {
        return HttpResult(-1);
      }
    } else {
      return result;
    }
  }

  Future<HttpResult> refreshSession() async {
    String? refreshSession = window.localStorage["refresh_session"];
    if (refreshSession == null) {
      return HttpResult(-1);
    }

    var (result, resp) = await _client.postJson("/account/refresh_session", {}, headers: {"authorization": "Bearer $refreshSession"});
    if (result.isSuccess) {
      if (resp is Map) {
        if (resp["err"] == 0) {
          var data = resp["result"] as Map;
          window.localStorage["session"] = data["session"];
          window.localStorage["refresh_session"] = data["refresh_session"];
          return HttpResult(0);
        } else {
          return HttpResult(resp["err"], msg: resp["msg"] as String);
        }
      } else {
        return HttpResult(-1);
      }
    } else {
      return result;
    }
  }
}
