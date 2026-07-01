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

TrafficStats _$TrafficStatsFromJson(Map<String, dynamic> json) => TrafficStats(
      txBytes: json['tx_bytes'] as String,
      txSpeed: json['tx_speed'] as String,
      rxBytes: json['rx_bytes'] as String,
      rxSpeed: json['rx_speed'] as String,
    );

Map<String, dynamic> _$TrafficStatsToJson(TrafficStats instance) =>
    <String, dynamic>{
      'tx_bytes': instance.txBytes,
      'tx_speed': instance.txSpeed,
      'rx_bytes': instance.rxBytes,
      'rx_speed': instance.rxSpeed,
    };

PnServerInfo _$PnServerInfoFromJson(Map<String, dynamic> json) => PnServerInfo(
      id: json['id'] as String,
      ip: json['ip'] as String,
      port: (json['port'] as num).toInt(),
    );

Map<String, dynamic> _$PnServerInfoToJson(PnServerInfo instance) =>
    <String, dynamic>{
      'id': instance.id,
      'ip': instance.ip,
      'port': instance.port,
    };

ProxyNode _$ProxyNodeFromJson(Map<String, dynamic> json) => ProxyNode(
      pnServer:
          PnServerInfo.fromJson(json['pn_server'] as Map<String, dynamic>),
      observedAddr: json['observed_addr'] as String?,
      status: json['status'] as String,
      live: json['live'] as bool,
      updatedAt: json['updated_at'] as String,
      comment: json['comment'] as String?,
    );

Map<String, dynamic> _$ProxyNodeToJson(ProxyNode instance) => <String, dynamic>{
      'pn_server': instance.pnServer,
      'observed_addr': instance.observedAddr,
      'status': instance.status,
      'live': instance.live,
      'updated_at': instance.updatedAt,
      'comment': instance.comment,
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
      txBytes: json['tx_bytes'] as String,
      txSpeed: json['tx_speed'] as String,
      rxBytes: json['rx_bytes'] as String,
      rxSpeed: json['rx_speed'] as String,
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
      'tx_bytes': instance.txBytes,
      'tx_speed': instance.txSpeed,
      'rx_bytes': instance.rxBytes,
      'rx_speed': instance.rxSpeed,
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
      txBytes: json['tx_bytes'] as String,
      txSpeed: json['tx_speed'] as String,
      rxBytes: json['rx_bytes'] as String,
      rxSpeed: json['rx_speed'] as String,
    );

Map<String, dynamic> _$NetworkMemberToJson(NetworkMember instance) =>
    <String, dynamic>{
      'id': instance.nodeId,
      'ip': instance.ipAddr,
      'online': instance.isOnline,
      'client_version': instance.clientVersion,
      'ip_list': instance.ipList,
      'tx_bytes': instance.txBytes,
      'tx_speed': instance.txSpeed,
      'rx_bytes': instance.rxBytes,
      'rx_speed': instance.rxSpeed,
    };
