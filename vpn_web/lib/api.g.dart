// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'api.dart';

// **************************************************************************
// JsonSerializableGenerator
// **************************************************************************

LoginReq _$LoginReqFromJson(Map<String, dynamic> json) => LoginReq(
      userName: json['user_name'] as String,
      password: json['password'] as String,
      timestamp: (json['timestamp'] as num).toInt(),
    );

Map<String, dynamic> _$LoginReqToJson(LoginReq instance) => <String, dynamic>{
      'user_name': instance.userName,
      'password': instance.password,
      'timestamp': instance.timestamp,
    };

UserInfo _$UserInfoFromJson(Map<String, dynamic> json) => UserInfo(
      id: json['id'] as String,
      networkId: json['network_id'] as String,
      sessionId: json['session_id'] as String,
      serverId: json['server_id'] as String,
    );

Map<String, dynamic> _$UserInfoToJson(UserInfo instance) => <String, dynamic>{
      'id': instance.id,
      'network_id': instance.networkId,
      'session_id': instance.sessionId,
      'server_id': instance.serverId,
    };

JoinedNode _$JoinedNodeFromJson(Map<String, dynamic> json) => JoinedNode(
      groupId: json['group_id'] as String,
      nodeId: json['node_id'] as String,
      allowJoin: json['allow_join'] as bool,
      name: json['name'] as String,
      comment: json['comment'] as String,
      isOnline: json['online'] as bool,
      clientVersion: json['client_version'] as String?,
      ipList:
          (json['ip_list'] as List<dynamic>?)?.map((e) => e as String).toList(),
    );

Map<String, dynamic> _$JoinedNodeToJson(JoinedNode instance) =>
    <String, dynamic>{
      'group_id': instance.groupId,
      'node_id': instance.nodeId,
      'allow_join': instance.allowJoin,
      'name': instance.name,
      'comment': instance.comment,
      'online': instance.isOnline,
      'client_version': instance.clientVersion,
      'ip_list': instance.ipList,
    };

Network _$NetworkFromJson(Map<String, dynamic> json) => Network(
      id: json['id'] as String,
      groupId: json['group_id'] as String,
      name: json['name'] as String,
      ipSeg: json['ip_seg'] as String?,
      mask: (json['mask'] as num).toInt(),
      ipv6Seg: json['ipv6_seg'] as String?,
      ipv6Mask: (json['ipv6_mask'] as num).toInt(),
    );

Map<String, dynamic> _$NetworkToJson(Network instance) => <String, dynamic>{
      'id': instance.id,
      'group_id': instance.groupId,
      'name': instance.name,
      'ip_seg': instance.ipSeg,
      'mask': instance.mask,
      'ipv6_seg': instance.ipv6Seg,
      'ipv6_mask': instance.ipv6Mask,
    };

NetworkMember _$NetworkMemberFromJson(Map<String, dynamic> json) =>
    NetworkMember(
      nodeId: json['id'] as String,
      ipAddr: json['ip'] as String,
      isOnline: json['online'] as bool,
      clientVersion: json['client_version'] as String?,
      ipList:
          (json['ip_list'] as List<dynamic>?)?.map((e) => e as String).toList(),
    );

Map<String, dynamic> _$NetworkMemberToJson(NetworkMember instance) =>
    <String, dynamic>{
      'id': instance.nodeId,
      'ip': instance.ipAddr,
      'online': instance.isOnline,
      'client_version': instance.clientVersion,
      'ip_list': instance.ipList,
    };
