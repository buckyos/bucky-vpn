import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:vpn_web/edit_network_dialog.dart';
import 'package:vpn_web/network_members_page.dart';

import 'api.dart';

class NetworksPage extends StatefulWidget {
  const NetworksPage({super.key});

  @override
  createState() => _NetworksPageState();
}

class _NetworksPageState extends State<NetworksPage> {
  List<Network>? _networks;

  @override
  void initState() {
    super.initState();

    Api.instance().getNetworks().then((ret) {
      var (result, resp) = ret;
      if (result.isSuccess) {
        if (mounted) {
          setState(() {
            _networks = resp ?? [];
          });
        }
      } else {
        Fluttertoast.showToast(
            msg: result.msg ?? "获取网络列表失败",
            toastLength: Toast.LENGTH_LONG,
            gravity: ToastGravity.TOP,
            backgroundColor: Colors.red,
            textColor: Colors.white,
            fontSize: 16.0,
            timeInSecForIosWeb: 5);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    if (_networks == null) {
      return const Scaffold(
        body: Center(
          child: CircularProgressIndicator(),
        ),
      );
    } else {
      return Scaffold(
        backgroundColor: Colors.white,
        body: Container(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              SizedBox(
                height: 40,
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.start,
                  children: [
                    SizedBox(
                      height: 40,
                      width: 120,
                      child: ElevatedButton(
                          onPressed: () async {
                            showDialog(
                                context: context,
                                builder: (context) {
                                  return EditNetworkDialog(
                                    name: "test",
                                    address: "192.168.8.0",
                                    mask: 24,
                                    onSave: (name, address, mask) async {
                                      var result = await Api.instance()
                                          .addNetwork(name, address, mask);
                                      if (result.isSuccess) {
                                        var (ret, resp) =
                                            await Api.instance().getNetworks();
                                        if (ret.isSuccess) {
                                          if (mounted) {
                                            setState(() {
                                              _networks = resp ?? [];
                                            });
                                          }
                                        }
                                      } else {
                                        Fluttertoast.showToast(
                                            msg: result.msg ?? "新建网络失败",
                                            toastLength: Toast.LENGTH_LONG,
                                            gravity: ToastGravity.TOP,
                                            backgroundColor: Colors.red,
                                            textColor: Colors.white,
                                            fontSize: 16.0,
                                            timeInSecForIosWeb: 5);
                                      }
                                    },
                                  );
                                });
                          },
                          child: Text("新建网络")),
                    )
                  ],
                ),
              ),
              const SizedBox(
                height: 20,
              ),
              Expanded(
                  child: Table(
                      border: TableBorder.all(color: Colors.black),
                      defaultVerticalAlignment:
                          TableCellVerticalAlignment.middle,
                      columnWidths: {
                    0: FixedColumnWidth(120),
                    1: FlexColumnWidth(),
                    2: FixedColumnWidth(120),
                    3: FixedColumnWidth(200),
                  },
                      children: [
                    TableRow(
                      children: [
                        TableCell(
                            child: Center(
                          child: Text("Name"),
                        )),
                        TableCell(
                            child: Center(
                          child: Text("Address"),
                        )),
                        TableCell(
                            child: Center(
                          child: Text("Mask"),
                        )),
                        TableCell(
                            child: Center(
                          child: Text("Operation"),
                        )),
                      ],
                    ),
                    for (var network in _networks!)
                      TableRow(
                        children: [
                          TableCell(
                              child: Center(
                            child: Text(network.name),
                          )),
                          TableCell(
                              child: Center(
                            child: Text(network.ipSeg!),
                          )),
                          TableCell(
                              child: Center(
                            child: Text(network.mask.toString()),
                          )),
                          TableCell(
                              child: Center(
                            child: MouseRegion(
                                cursor: SystemMouseCursors.click,
                                child: InkWell(
                                    onTap: () async {
                                      showDialog(
                                          context: context,
                                          builder: (context) {
                                            return Dialog(
                                              child: SizedBox(
                                                width: 1024,
                                                child: NetworkMembersPage(
                                                    network: network),
                                              ),
                                            );
                                          });
                                    },
                                    child: Text(
                                      "查看",
                                      style: TextStyle(
                                          color: Colors.blue,
                                          decoration: TextDecoration.underline),
                                    ))),
                          )),
                        ],
                      )
                  ]))
            ],
          ),
        ),
      );
    }
  }
}
