import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:vpn_web/dialog_button.dart';
import 'package:vpn_web/edit_network_dialog.dart';
import 'package:vpn_web/network_members_page.dart';
import 'package:vpn_web/prompt_dialog.dart';

import 'api.dart';

class NetworksPage extends StatefulWidget {
  const NetworksPage({super.key});

  @override
  createState() => _NetworksPageState();
}

class _NetworksPageState extends State<NetworksPage> {
  List<Network>? _networks;

  void refreshNetworks() {
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
            msg: result.msg ?? "Read networks failed",
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
  void initState() {
    super.initState();
    refreshNetworks();
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
                height: 10,
              ),
              Row(
                mainAxisAlignment: MainAxisAlignment.start,
                children: [
                  DialogButton(
                    onPressed: () async {
                      showDialog(
                          context: context,
                          builder: (context) {
                            return EditNetworkDialog(
                              name: "test",
                              address: "192.168.18.0",
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
                                      msg: result.msg ?? "New network failed",
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
                    text: 'New',
                  )
                ],
              ),
              const SizedBox(
                height: 10,
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
                          child: Text("Action"),
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
                              child: Row(
                                  mainAxisAlignment:
                                      MainAxisAlignment.spaceAround,
                                  children: [
                                MouseRegion(
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
                                          "view",
                                          style: TextStyle(
                                              color: Colors.blue,
                                              decoration:
                                                  TextDecoration.underline),
                                        ))),
                                    MouseRegion(
                                        cursor: SystemMouseCursors.click,
                                        child: InkWell(
                                            onTap: () async {
                                              showDialog(
                                                  context: context,
                                                  builder: (context) {
                                                    return EditNetworkDialog(
                                                      name: network.name,
                                                      address: network.ipSeg!,
                                                      mask: network.mask,
                                                      onSave: (name, address, mask) async {
                                                        var result = await Api.instance()
                                                            .updateNetwork(network.id, name, address, mask);
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
                                                              msg: result.msg ?? "Edit network failed",
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
                                            child: Text(
                                              "edit",
                                              style: TextStyle(
                                                  color: Colors.blue,
                                                  decoration:
                                                  TextDecoration.underline),
                                            ))),
                                MouseRegion(
                                    cursor: SystemMouseCursors.click,
                                    child: InkWell(
                                        onTap: () async {
                                          showDialog(
                                              context: context,
                                              builder: (context) {
                                                return Dialog(
                                                  backgroundColor:
                                                      Colors.transparent,
                                                  child: SizedBox(
                                                      width: 400,
                                                      height: 300,
                                                      child: PromptDialog(
                                                        promptTitle:
                                                            "Delete Network",
                                                        prompt:
                                                            "Are you sure to delete networ ${network.name}?",
                                                        onConfirm: () async {
                                                          var result = await Api
                                                                  .instance()
                                                              .deleteNetwork(
                                                                  network.id);
                                                          if (result
                                                              .isSuccess) {
                                                            refreshNetworks();
                                                          } else {
                                                            Fluttertoast.showToast(
                                                                msg: result
                                                                        .msg ??
                                                                    "delete failed",
                                                                toastLength: Toast
                                                                    .LENGTH_LONG,
                                                                gravity:
                                                                    ToastGravity
                                                                        .TOP,
                                                                backgroundColor:
                                                                    Colors.red,
                                                                textColor:
                                                                    Colors
                                                                        .white,
                                                                fontSize: 16.0,
                                                                timeInSecForIosWeb:
                                                                    5);
                                                          }
                                                        },
                                                      )),
                                                );
                                              });
                                        },
                                        child: Text(
                                          "delete",
                                          style: TextStyle(
                                              color: Colors.blue,
                                              decoration:
                                                  TextDecoration.underline),
                                        ))),
                              ])),
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
