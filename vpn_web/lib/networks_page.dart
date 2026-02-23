import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';

import 'api.dart';
import 'edit_network_dialog.dart';
import 'network_members_page.dart';
import 'prompt_dialog.dart';

class NetworksPage extends StatefulWidget {
  const NetworksPage({super.key});

  @override
  State<NetworksPage> createState() => _NetworksPageState();
}

class _NetworksPageState extends State<NetworksPage> {
  List<Network>? _networks;

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

  Future<void> refreshNetworks() async {
    final (result, resp) = await Api.instance().getNetworks();
    if (!mounted) {
      return;
    }

    if (result.isSuccess) {
      setState(() {
        _networks = resp ?? [];
      });
      return;
    }

    Fluttertoast.showToast(
      msg: result.msg ?? 'Read networks failed',
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
    refreshNetworks();
  }

  @override
  Widget build(BuildContext context) {
    if (_networks == null) {
      return const Center(child: CircularProgressIndicator());
    }

    return Column(
      children: [
        Row(
          children: [
            const Icon(Icons.device_hub_outlined,
                size: 18, color: Color(0xFF4B6675)),
            const SizedBox(width: 8),
            const Text(
              'Networks',
              style: TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.w700,
                color: Color(0xFF204153),
              ),
            ),
            const Spacer(),
            FilledButton.icon(
              onPressed: () {
                showDialog(
                  context: context,
                  builder: (_) => EditNetworkDialog(
                    name: '',
                    address: '192.168.18.0',
                    mask: 24,
                    onSave: (name, address, mask) async {
                      final result =
                          await Api.instance().addNetwork(name, address, mask);
                      if (result.isSuccess) {
                        refreshNetworks();
                        return;
                      }
                      Fluttertoast.showToast(
                        msg: result.msg ?? 'New network failed',
                        toastLength: Toast.LENGTH_LONG,
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
              icon: const Icon(Icons.add, size: 18),
              label: const Text('New Network'),
            ),
            const SizedBox(width: 10),
            OutlinedButton.icon(
              onPressed: refreshNetworks,
              icon: const Icon(Icons.refresh, size: 18),
              label: const Text('Refresh'),
            ),
            const SizedBox(width: 10),
            Text(
              '${_networks!.length} networks',
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
                      columns: const [
                        DataColumn(label: Text('Name')),
                        DataColumn(label: Text('Address')),
                        DataColumn(label: Text('Mask')),
                        DataColumn(label: Text('Action')),
                      ],
                      rows: _networks!
                          .map(
                            (network) => DataRow(
                              cells: [
                                DataCell(Text(network.name)),
                                DataCell(Text(network.ipSeg ?? '-')),
                                DataCell(Text(network.mask.toString())),
                                DataCell(
                                  Wrap(
                                    spacing: 8,
                                    children: [
                                      _actionLink(
                                        label: 'Members',
                                        onTap: () {
                                          showDialog(
                                            context: context,
                                            builder: (_) => Dialog(
                                              backgroundColor:
                                                  Colors.transparent,
                                              child: Container(
                                                constraints:
                                                    const BoxConstraints(
                                                        maxWidth: 760),
                                                padding:
                                                    const EdgeInsets.all(12),
                                                decoration: BoxDecoration(
                                                  color: Colors.white,
                                                  borderRadius:
                                                      BorderRadius.circular(16),
                                                ),
                                                child: NetworkMembersPage(
                                                    network: network),
                                              ),
                                            ),
                                          );
                                        },
                                      ),
                                      _actionLink(
                                        label: 'Edit',
                                        onTap: () {
                                          showDialog(
                                            context: context,
                                            builder: (_) => EditNetworkDialog(
                                              name: network.name,
                                              address: network.ipSeg,
                                              mask: network.mask,
                                              onSave:
                                                  (name, address, mask) async {
                                                final result =
                                                    await Api.instance()
                                                        .updateNetwork(
                                                  network.id,
                                                  name,
                                                  address,
                                                  mask,
                                                );
                                                if (result.isSuccess) {
                                                  refreshNetworks();
                                                  return;
                                                }
                                                Fluttertoast.showToast(
                                                  msg: result.msg ??
                                                      'Edit network failed',
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
                                      _actionLink(
                                        label: 'Delete',
                                        color: const Color(0xFFB42318),
                                        onTap: () {
                                          showDialog(
                                            context: context,
                                            builder: (_) => PromptDialog(
                                              promptTitle: 'Delete Network',
                                              prompt:
                                                  'Are you sure to delete network ${network.name}?',
                                              onConfirm: () async {
                                                final result =
                                                    await Api.instance()
                                                        .deleteNetwork(
                                                            network.id);
                                                if (result.isSuccess) {
                                                  refreshNetworks();
                                                  return;
                                                }
                                                Fluttertoast.showToast(
                                                  msg: result.msg ??
                                                      'Delete network failed',
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
