import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';

import 'api.dart';
import 'input_dialog.dart';
import 'prompt_dialog.dart';
import 'traffic_stats.dart';

const double _trafficCellWidth = 100;
const double _trafficLabelWidth = 36;
const double _trafficValueGap = 5;
const double _tableColumnSpacing = 28;
const double _tableHorizontalMargin = 16;

class JoinedNodesPage extends StatefulWidget {
  const JoinedNodesPage({super.key});

  @override
  State<JoinedNodesPage> createState() => _JoinedNodesPageState();
}

class _JoinedNodesPageState extends State<JoinedNodesPage> {
  List<JoinedNode>? _joinedNodes;

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

  Future<void> refreshNodes() async {
    final (result, resp) = await Api.instance().getJoinedNodes();
    if (!mounted) {
      return;
    }

    if (result.isSuccess) {
      setState(() {
        _joinedNodes = resp ?? [];
      });
      return;
    }

    Fluttertoast.showToast(
      msg: result.msg ?? 'Read nodes failed',
      toastLength: Toast.LENGTH_LONG,
      gravity: ToastGravity.TOP,
      backgroundColor: Colors.red,
      textColor: Colors.white,
      fontSize: 16.0,
      timeInSecForIosWeb: 5,
    );
  }

  @override
  void initState() {
    super.initState();
    refreshNodes();
  }

  @override
  Widget build(BuildContext context) {
    if (_joinedNodes == null) {
      return const Center(child: CircularProgressIndicator());
    }

    return Column(
      children: [
        Row(
          children: [
            const Icon(Icons.hub_outlined, size: 18, color: Color(0xFF4B6675)),
            const SizedBox(width: 8),
            const Text(
              'Joined Nodes',
              style: TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.w700,
                color: Color(0xFF204153),
              ),
            ),
            const Spacer(),
            FilledButton.icon(
              onPressed: refreshNodes,
              icon: const Icon(Icons.refresh, size: 18),
              label: const Text('Refresh'),
            ),
            const SizedBox(width: 10),
            Text(
              '${_joinedNodes!.length} nodes',
              style: const TextStyle(color: Color(0xFF4B6675)),
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
                    constraints: BoxConstraints(minWidth: constraints.maxWidth),
                    child: DataTable(
                      columnSpacing: _tableColumnSpacing,
                      horizontalMargin: _tableHorizontalMargin,
                      dataRowMinHeight: 68,
                      dataRowMaxHeight: 68,
                      columns: const [
                        DataColumn(label: Text('Allow Join')),
                        DataColumn(label: Text('Name')),
                        DataColumn(label: Text('Node ID')),
                        DataColumn(label: Text('Speed')),
                        DataColumn(label: Text('Traffic')),
                        DataColumn(label: Text('Status')),
                        DataColumn(label: Text('Action')),
                      ],
                      rows: _joinedNodes!
                          .map(
                            (node) => DataRow(
                              cells: [
                                DataCell(
                                  Checkbox(
                                    value: node.allowJoin,
                                    activeColor: const Color(0xFF0A7E8C),
                                    onChanged: (value) async {
                                      if (value == null) {
                                        return;
                                      }
                                      final result = await Api.instance()
                                          .allowJoin(node.nodeId, value);
                                      if (result.isSuccess) {
                                        setState(() {
                                          node.allowJoin = value;
                                        });
                                      }
                                    },
                                  ),
                                ),
                                DataCell(
                                  Text(
                                    node.comment.isNotEmpty
                                        ? node.comment
                                        : (node.name.isNotEmpty
                                            ? node.name
                                            : node.nodeId),
                                  ),
                                ),
                                DataCell(SizedBox(
                                    width: 320,
                                    child: SelectableText(node.nodeId))),
                                DataCell(
                                  _buildTrafficCell(
                                    txValue: node.txSpeed,
                                    rxValue: node.rxSpeed,
                                    speed: true,
                                  ),
                                ),
                                DataCell(
                                  _buildTrafficCell(
                                    txValue: node.txBytes,
                                    rxValue: node.rxBytes,
                                    speed: false,
                                  ),
                                ),
                                DataCell(
                                  Text(
                                    node.isOnline
                                        ? (node.ipList?.isNotEmpty == true
                                            ? node.ipList!.join(', ')
                                            : 'online')
                                        : 'offline',
                                    style: TextStyle(
                                      color: node.isOnline
                                          ? const Color(0xFF18794E)
                                          : const Color(0xFF8A3B12),
                                    ),
                                  ),
                                ),
                                DataCell(
                                  Wrap(
                                    spacing: 8,
                                    children: [
                                      _actionLink(
                                        label: 'Comment',
                                        onTap: () {
                                          showDialog(
                                            context: context,
                                            builder: (context) => InputDialog(
                                              onOk: (comment) async {
                                                final result =
                                                    await Api.instance()
                                                        .updateJoinComment(
                                                            node.nodeId,
                                                            comment);
                                                if (result.isSuccess) {
                                                  refreshNodes();
                                                  return;
                                                }
                                                Fluttertoast.showToast(
                                                  msg: result.msg ??
                                                      'Update comment failed',
                                                  toastLength:
                                                      Toast.LENGTH_LONG,
                                                  gravity: ToastGravity.TOP,
                                                  backgroundColor: Colors.red,
                                                  textColor: Colors.white,
                                                  fontSize: 16.0,
                                                  timeInSecForIosWeb: 5,
                                                );
                                              },
                                              defaultContent: node.comment,
                                              hintText: 'Enter comment',
                                              title: 'Comment',
                                            ),
                                          );
                                        },
                                      ),
                                      _actionLink(
                                        label: 'Delete',
                                        color: const Color(0xFFB42318),
                                        onTap: () {
                                          showDialog(
                                            context: context,
                                            builder: (context) => PromptDialog(
                                              promptTitle: 'Delete Node',
                                              prompt:
                                                  'Are you sure to delete node ${node.name.isEmpty ? node.nodeId : node.name}?',
                                              onConfirm: () async {
                                                final result =
                                                    await Api.instance()
                                                        .deleteJoinedNode(
                                                            node.nodeId);
                                                if (result.isSuccess) {
                                                  refreshNodes();
                                                  return;
                                                }
                                                Fluttertoast.showToast(
                                                  msg: result.msg ??
                                                      'Delete node failed',
                                                  toastLength:
                                                      Toast.LENGTH_LONG,
                                                  gravity: ToastGravity.TOP,
                                                  backgroundColor: Colors.red,
                                                  textColor: Colors.white,
                                                  fontSize: 16.0,
                                                  timeInSecForIosWeb: 5,
                                                );
                                              },
                                            ),
                                          );
                                        },
                                      ),
                                    ],
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
      ],
    );
  }
}
