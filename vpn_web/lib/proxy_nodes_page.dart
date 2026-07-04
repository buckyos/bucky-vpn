import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';

import 'api.dart';
import 'input_dialog.dart';

const double _tableColumnSpacing = 28;
const double _tableHorizontalMargin = 16;

class ProxyNodesPage extends StatefulWidget {
  const ProxyNodesPage({super.key});

  @override
  State<ProxyNodesPage> createState() => _ProxyNodesPageState();
}

class _ProxyNodesPageState extends State<ProxyNodesPage> {
  List<ProxyNode>? _proxyNodes;

  String _nodeAddress(ProxyNode node) {
    final addresses =
        node.pnServer.allAddresses.map((addr) => addr.display).toList();
    final observedAddr = node.observedAddr ?? '';
    if (observedAddr.isNotEmpty && !addresses.contains(observedAddr)) {
      addresses.insert(0, observedAddr);
    }
    return addresses.join(', ');
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
    final (result, resp) = await Api.instance().getProxyNodes();
    if (!mounted) {
      return;
    }

    if (result.isSuccess) {
      setState(() {
        _proxyNodes = resp ?? [];
      });
      return;
    }

    _showError(result.msg ?? 'Read proxy nodes failed');
  }

  Future<void> _setAllowed(
    ProxyNode node,
    bool allowed, {
    String? comment,
  }) async {
    final result = allowed
        ? await Api.instance().approveProxyNode(
            node.pnServer,
            comment: comment,
          )
        : await Api.instance().rejectProxyNode(
            node.pnServer,
            comment: comment,
          );

    if (!mounted) {
      return;
    }

    if (result.isSuccess) {
      await refreshNodes();
      return;
    }

    _showError(result.msg ?? 'Update proxy node status failed');
  }

  void _promptComment(ProxyNode node) {
    if (node.status != 'approved' && node.status != 'rejected') {
      _showError('Set proxy approval before adding a comment');
      return;
    }

    showDialog(
      context: context,
      builder: (context) => InputDialog(
        onOk: (comment) => _setAllowed(node, node.isAllowed, comment: comment),
        defaultContent: node.comment ?? '',
        hintText: 'Enter comment',
        title: 'Comment',
      ),
    );
  }

  void _showError(String message) {
    Fluttertoast.showToast(
      msg: message,
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
    if (_proxyNodes == null) {
      return const Center(child: CircularProgressIndicator());
    }

    return Column(
      children: [
        Row(
          children: [
            const Icon(Icons.router_outlined,
                size: 18, color: Color(0xFF4B6675)),
            const SizedBox(width: 8),
            const Text(
              'Proxy Nodes',
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
              '${_proxyNodes!.length} nodes',
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
                      dataRowMinHeight: 58,
                      dataRowMaxHeight: 58,
                      columns: const [
                        DataColumn(label: Text('Allowed')),
                        DataColumn(label: Text('Node ID')),
                        DataColumn(label: Text('Address')),
                        DataColumn(label: Text('Live')),
                        DataColumn(label: Text('Comment')),
                        DataColumn(label: Text('Action')),
                      ],
                      rows: _proxyNodes!
                          .map(
                            (node) => DataRow(
                              cells: [
                                DataCell(
                                  Checkbox(
                                    value: node.isAllowed,
                                    activeColor: const Color(0xFF0A7E8C),
                                    onChanged: (value) {
                                      if (value == null) {
                                        return;
                                      }
                                      _setAllowed(
                                        node,
                                        value,
                                        comment: node.comment,
                                      );
                                    },
                                  ),
                                ),
                                DataCell(
                                  SizedBox(
                                    width: 320,
                                    child: SelectableText(node.pnServer.id),
                                  ),
                                ),
                                DataCell(SelectableText(_nodeAddress(node))),
                                DataCell(
                                  Text(
                                    node.live ? 'online' : 'offline',
                                    style: TextStyle(
                                      color: node.live
                                          ? const Color(0xFF18794E)
                                          : const Color(0xFF8A3B12),
                                    ),
                                  ),
                                ),
                                DataCell(Text(
                                  node.comment?.isNotEmpty == true
                                      ? node.comment!
                                      : '-',
                                )),
                                DataCell(
                                  Wrap(
                                    spacing: 8,
                                    children: [
                                      _actionLink(
                                        label: 'Comment',
                                        onTap: () => _promptComment(node),
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
