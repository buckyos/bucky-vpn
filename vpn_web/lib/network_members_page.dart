import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';

import 'api.dart';

class NetworkMembersPage extends StatefulWidget {
  final Network network;

  const NetworkMembersPage({super.key, required this.network});

  @override
  createState() => _NetworkMembersPageState();
}

class _NetworkMembersPageState extends State<NetworkMembersPage> {
  late List<NetworkMember> _networkMembers = [];
  late List<JoinedNode> _joinedNodes = [];
  JoinedNode? addingNode;
  TextEditingController _ipController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _ipController.text = widget.network.ipSeg ?? "";
    Api.instance().getNetworkMember(widget.network.id).then((ret) {
      var (result, resp) = ret;
      if (result.isSuccess) {
        if (mounted) {
          setState(() {
            _networkMembers = resp ?? [];
          });
        }
      } else {
        Fluttertoast.showToast(
            msg: result.msg ?? "获取网络成员列表失败",
            toastLength: Toast.LENGTH_LONG,
            gravity: ToastGravity.TOP,
            backgroundColor: Colors.red,
            textColor: Colors.white,
            fontSize: 16.0,
            timeInSecForIosWeb: 5);
      }
    });
    Api.instance().getJoinedNodes().then((ret) {
      var (result, resp) = ret;
      if (result.isSuccess) {
        if (mounted) {
          setState(() {
            _joinedNodes = resp ?? [];
          });
        }
      }
    });
  }

  String? getNodeName(String nodeId) {
    for (var node in _joinedNodes) {
      if (node.nodeId == nodeId) {
        return node.comment.isNotEmpty ? node.comment : node.name.isNotEmpty? node.name : node.nodeId;
      }
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.white,
      body: Center(
        child: Container(
          width: 600,
          child: Column(
            mainAxisAlignment: MainAxisAlignment.start,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              SizedBox(
                height: 40,
              ),
              Table(
                      border: TableBorder.all(color: Colors.black),
                      defaultVerticalAlignment:
                      TableCellVerticalAlignment.middle,
                      children: [
                TableRow(children: [
                  TableCell(child: Center(child: Text("Name"))),
                  TableCell(child: Center(child: Text("Ip"))),
                  TableCell(child: Center(child: Text("Action"))),
                ]),
                for (var member in _networkMembers)
                  TableRow(children: [
                    TableCell(child: Center(child: SelectableText(getNodeName(member.nodeId)?? member.nodeId))),
                        TableCell(
                            child:
                                Center(child: SelectableText(member.ipAddr))),
                        TableCell(
                            child: Center(
                                child: MouseRegion(
                                    cursor: SystemMouseCursors.click,
                                    child: InkWell(
                                      onTap: () async {
                                        final result = await Api.instance()
                                            .deleteNetworkMember(
                                                widget.network.id,
                                                member.nodeId);
                                        if (result.isSuccess) {
                                          Fluttertoast.showToast(
                                            msg: "Remove member success",
                                            toastLength: Toast.LENGTH_SHORT,
                                            gravity: ToastGravity.TOP,
                                            backgroundColor: Colors.black,
                                            textColor: Colors.white,
                                            fontSize: 16.0,
                                          );
                                          setState(() {
                                            _networkMembers.remove(member);
                                          });
                                        } else {
                                          Fluttertoast.showToast(
                                            msg: result.msg ??
                                                "Remove member failed",
                                            toastLength: Toast.LENGTH_SHORT,
                                            gravity: ToastGravity.TOP,
                                            backgroundColor: Colors.black,
                                            textColor: Colors.white,
                                            fontSize: 16.0,
                                          );
                                        }
                                      },
                                      child: Text(
                                        "Remove",
                                        style: TextStyle(
                                            color: Colors.blue,
                                            decoration:
                                                TextDecoration.underline),
                                      ),
                                    ))))
                      ]),
                    TableRow(children: [
                      TableCell(
                    child: Center(
                        child: Container(
                          padding: EdgeInsets.all(3),
                      height: 40,
                      child: DropdownButton<JoinedNode>(
                        value: addingNode,
                        onChanged: (JoinedNode? newValue) {
                          setState(() {
                            addingNode = newValue;
                          });
                        },
                        items: _joinedNodes
                            .map<DropdownMenuItem<JoinedNode>>((JoinedNode value) {
                          return DropdownMenuItem<JoinedNode>(
                            value: value,
                            child: Text(value.name),
                          );
                        }).toList(),
                      ),
                    )),
                  ),
                  TableCell(
                    child: Center(
                        child: Container(
                          padding: EdgeInsets.all(3),
                      height: 40,
                      child: TextField(
                        controller: _ipController,
                        textAlignVertical: TextAlignVertical.center,
                        decoration: const InputDecoration(
                          border: OutlineInputBorder(),
                          hintText: 'IP',
                        ),
                      ),
                    )),
                  ),
                  TableCell(
                      child: Center(
                    child: MouseRegion(
                        cursor: SystemMouseCursors.click,
                        child: InkWell(
                            onTap: () async {
                              if (addingNode == null) {
                                Fluttertoast.showToast(
                                  msg: "Please select a node",
                                  toastLength: Toast.LENGTH_SHORT,
                                  gravity: ToastGravity.TOP,
                                  backgroundColor: Colors.black,
                                  textColor: Colors.white,
                                  fontSize: 16.0,
                                );
                                return;
                              }

                              if (_networkMembers.where((member) => member.nodeId == addingNode!.nodeId).toList().length == 1) {
                                Fluttertoast.showToast(
                                  msg: "Node already in network",
                                  toastLength: Toast.LENGTH_SHORT,
                                  gravity: ToastGravity.TOP,
                                  backgroundColor: Colors.black,
                                  textColor: Colors.white,
                                  fontSize: 16.0,
                                );
                                return;
                              }

                              final result = await Api.instance().addNetworkMember(
                                  widget.network.id, addingNode!.nodeId, _ipController.text);
                              if (result.isSuccess) {
                                Fluttertoast.showToast(
                                  msg: "Add member success",
                                  toastLength: Toast.LENGTH_SHORT,
                                  gravity: ToastGravity.TOP,
                                  backgroundColor: Colors.black,
                                  textColor: Colors.white,
                                  fontSize: 16.0,
                                );
                                setState(() {
                                  _networkMembers.add(NetworkMember(
                                      nodeId: addingNode!.nodeId,
                                      ipAddr: _ipController.text, isOnline: false));
                                });
                              } else {
                                Fluttertoast.showToast(
                                  msg: result.msg ?? "Add member failed",
                                  toastLength: Toast.LENGTH_SHORT,
                                  gravity: ToastGravity.TOP,
                                  backgroundColor: Colors.black,
                                  textColor: Colors.white,
                                  fontSize: 16.0,
                                );
                              }
                            },
                            child: Text(
                              "Add",
                              style: TextStyle(
                                  color: Colors.blue,
                                  decoration: TextDecoration.underline),
                            ))),
                  ))
                ])
              ])
            ],
          ),
        ),
      ),
    );
  }
}
