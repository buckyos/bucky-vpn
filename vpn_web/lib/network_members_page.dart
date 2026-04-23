import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';

import 'api.dart';
import 'traffic_stats.dart';

const double _trafficCellWidth = 100;
const double _trafficLabelWidth = 36;
const double _trafficValueGap = 5;
const double _tableColumnSpacing = 28;
const double _tableHorizontalMargin = 16;

class NetworkMembersPage extends StatefulWidget {
  final Network network;

  const NetworkMembersPage({super.key, required this.network});

  @override
  State<NetworkMembersPage> createState() => _NetworkMembersPageState();
}

class _NetworkMembersPageState extends State<NetworkMembersPage> {
  List<NetworkMember> _networkMembers = [];
  List<JoinedNode> _joinedNodes = [];
  JoinedNode? addingNode;
  final TextEditingController _ipController = TextEditingController();

  Widget _buildTrafficCell({
    required String txValue,
    required String rxValue,
    required bool speed,
  }) {
    final uploadValue =
        speed ? formatTrafficSpeed(txValue) : formatTrafficBytes(txValue);
    final downloadValue =
        speed ? formatTrafficSpeed(rxValue) : formatTrafficBytes(rxValue);

    return SizedBox(
      width: _trafficCellWidth,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const SizedBox(
                width: _trafficLabelWidth,
                child: Text(
                  'Up',
                  style: TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: Color(0xFF204153),
                  ),
                ),
              ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(left: _trafficValueGap),
                  child: Text(
                    uploadValue,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: Color(0xFF204153),
                    ),
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 4),
          Row(
            children: [
              const SizedBox(
                width: _trafficLabelWidth,
                child: Text(
                  'Down',
                  style: TextStyle(
                    fontSize: 13,
                    color: Color(0xFF4B6675),
                  ),
                ),
              ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(left: _trafficValueGap),
                  child: Text(
                    downloadValue,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 13,
                      color: Color(0xFF4B6675),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _actionLink({
    required String label,
    required VoidCallback onTap,
    Color color = const Color(0xFF0A7E8C),
  }) {
    var isHovered = false;

    return StatefulBuilder(
      builder: (context, setState) {
        return MouseRegion(
          cursor: SystemMouseCursors.click,
          onEnter: (_) => setState(() => isHovered = true),
          onExit: (_) => setState(() => isHovered = false),
          child: GestureDetector(
            onTap: onTap,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 6),
              child: Text(
                label,
                style: TextStyle(
                  color: isHovered
                      ? Color.lerp(color, Colors.black, 0.28)!
                      : color,
                  decoration: TextDecoration.underline,
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  @override
  void initState() {
    super.initState();
    _ipController.text = widget.network.ipSeg ?? '';
    _loadData();
  }

  Future<void> _loadData() async {
    final (memberResult, members) =
        await Api.instance().getNetworkMember(widget.network.id);
    final (joinedResult, joinedNodes) = await Api.instance().getJoinedNodes();

    if (!mounted) {
      return;
    }

    if (memberResult.isSuccess) {
      setState(() {
        _networkMembers = members ?? [];
      });
    } else {
      Fluttertoast.showToast(
        msg: memberResult.msg ?? 'Read network member failed',
        toastLength: Toast.LENGTH_LONG,
        gravity: ToastGravity.TOP,
        backgroundColor: Colors.red,
        textColor: Colors.white,
        fontSize: 16.0,
        timeInSecForIosWeb: 5,
      );
    }

    if (joinedResult.isSuccess) {
      setState(() {
        _joinedNodes = joinedNodes ?? [];
      });
    }
  }

  String? getNodeName(String nodeId) {
    for (final node in _joinedNodes) {
      if (node.nodeId == nodeId) {
        return node.comment.isNotEmpty
            ? node.comment
            : (node.name.isNotEmpty ? node.name : node.nodeId);
      }
    }
    return null;
  }

  Future<void> _removeMember(NetworkMember member) async {
    final result = await Api.instance()
        .deleteNetworkMember(widget.network.id, member.nodeId);
    if (result.isSuccess) {
      Fluttertoast.showToast(
        msg: 'Remove member success',
        toastLength: Toast.LENGTH_SHORT,
        gravity: ToastGravity.TOP,
        backgroundColor: Colors.black,
        textColor: Colors.white,
        fontSize: 16.0,
      );
      if (mounted) {
        setState(() {
          _networkMembers.remove(member);
        });
      }
      return;
    }

    Fluttertoast.showToast(
      msg: result.msg ?? 'Remove member failed',
      toastLength: Toast.LENGTH_SHORT,
      gravity: ToastGravity.TOP,
      backgroundColor: Colors.black,
      textColor: Colors.white,
      fontSize: 16.0,
    );
  }

  Future<void> _addMember() async {
    if (addingNode == null) {
      Fluttertoast.showToast(
        msg: 'Please select a node',
        toastLength: Toast.LENGTH_SHORT,
        gravity: ToastGravity.TOP,
        backgroundColor: Colors.black,
        textColor: Colors.white,
        fontSize: 16.0,
      );
      return;
    }

    if (_networkMembers
        .where((member) => member.nodeId == addingNode!.nodeId)
        .isNotEmpty) {
      Fluttertoast.showToast(
        msg: 'Node already in network',
        toastLength: Toast.LENGTH_SHORT,
        gravity: ToastGravity.TOP,
        backgroundColor: Colors.black,
        textColor: Colors.white,
        fontSize: 16.0,
      );
      return;
    }

    final result = await Api.instance().addNetworkMember(
      widget.network.id,
      addingNode!.nodeId,
      _ipController.text,
    );

    if (result.isSuccess) {
      Fluttertoast.showToast(
        msg: 'Add member success',
        toastLength: Toast.LENGTH_SHORT,
        gravity: ToastGravity.TOP,
        backgroundColor: Colors.black,
        textColor: Colors.white,
        fontSize: 16.0,
      );
      setState(() {
        _networkMembers.add(
          NetworkMember(
            nodeId: addingNode!.nodeId,
            ipAddr: _ipController.text,
            isOnline: false,
            txBytes: '0',
            txSpeed: '0',
            rxBytes: '0',
            rxSpeed: '0',
          ),
        );
      });
      return;
    }

    Fluttertoast.showToast(
      msg: result.msg ?? 'Add member failed',
      toastLength: Toast.LENGTH_SHORT,
      gravity: ToastGravity.TOP,
      backgroundColor: Colors.black,
      textColor: Colors.white,
      fontSize: 16.0,
    );
  }

  @override
  void dispose() {
    _ipController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.group_outlined,
                  size: 18, color: Color(0xFF4B6675)),
              const SizedBox(width: 8),
              Text(
                'Members of ${widget.network.name}',
                style: const TextStyle(
                  fontSize: 18,
                  fontWeight: FontWeight.w700,
                  color: Color(0xFF0E2A3A),
                ),
              ),
              const Spacer(),
              OutlinedButton.icon(
                onPressed: _loadData,
                icon: const Icon(Icons.refresh, size: 18),
                label: const Text('Refresh'),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Expanded(
            child: Container(
              width: double.infinity,
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: const Color(0xFFF8FBFD),
                borderRadius: BorderRadius.circular(14),
                border: Border.all(color: const Color(0xFFD9E6EC)),
              ),
              child: LayoutBuilder(
                builder: (context, constraints) => SingleChildScrollView(
                  child: SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: ConstrainedBox(
                      constraints:
                          BoxConstraints(minWidth: constraints.maxWidth),
                      child: DataTable(
                        columnSpacing: _tableColumnSpacing,
                        horizontalMargin: _tableHorizontalMargin,
                        dataRowMinHeight: 68,
                        dataRowMaxHeight: 68,
                        columns: const [
                          DataColumn(label: Text('Name')),
                          DataColumn(label: Text('IP')),
                          DataColumn(label: Text('Speed')),
                          DataColumn(label: Text('Traffic')),
                          DataColumn(label: Text('Status')),
                          DataColumn(label: Text('Action')),
                        ],
                        rows: _networkMembers
                            .map(
                              (member) => DataRow(
                                cells: [
                                  DataCell(
                                    SelectableText(getNodeName(member.nodeId) ??
                                        member.nodeId),
                                  ),
                                  DataCell(SelectableText(member.ipAddr)),
                                  DataCell(
                                    _buildTrafficCell(
                                      txValue: member.txSpeed,
                                      rxValue: member.rxSpeed,
                                      speed: true,
                                    ),
                                  ),
                                  DataCell(
                                    _buildTrafficCell(
                                      txValue: member.txBytes,
                                      rxValue: member.rxBytes,
                                      speed: false,
                                    ),
                                  ),
                                  DataCell(
                                    Text(
                                      member.isOnline
                                          ? (member.ipList?.join(', ') ??
                                              'online')
                                          : 'offline',
                                      style: TextStyle(
                                        color: member.isOnline
                                            ? const Color(0xFF18794E)
                                            : const Color(0xFF8A3B12),
                                      ),
                                    ),
                                  ),
                                  DataCell(
                                    _actionLink(
                                      label: 'Remove',
                                      color: const Color(0xFFB42318),
                                      onTap: () => _removeMember(member),
                                    ),
                                  ),
                                ],
                              ),
                            )
                            .toList(),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
          const SizedBox(height: 12),
          Container(
            width: double.infinity,
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: const Color(0xFFF8FBFD),
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: const Color(0xFFD9E6EC)),
            ),
            child: Row(
              children: [
                Expanded(
                  flex: 3,
                  child: DropdownButtonFormField<JoinedNode>(
                    initialValue: addingNode,
                    decoration: InputDecoration(
                      labelText: 'Node',
                      filled: true,
                      fillColor: Colors.white,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(10),
                      ),
                    ),
                    items: _joinedNodes.map((node) {
                      return DropdownMenuItem<JoinedNode>(
                        value: node,
                        child: Text(
                          node.comment.isNotEmpty
                              ? node.comment
                              : (node.name.isNotEmpty
                                  ? node.name
                                  : node.nodeId),
                        ),
                      );
                    }).toList(),
                    onChanged: (newValue) {
                      setState(() {
                        addingNode = newValue;
                      });
                    },
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  flex: 2,
                  child: TextField(
                    controller: _ipController,
                    decoration: InputDecoration(
                      labelText: 'IP Address',
                      filled: true,
                      fillColor: Colors.white,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(10),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 10),
                FilledButton(
                  onPressed: _addMember,
                  style: FilledButton.styleFrom(
                    backgroundColor: const Color(0xFF0A7E8C),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10),
                    ),
                  ),
                  child: const Text('Add'),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
