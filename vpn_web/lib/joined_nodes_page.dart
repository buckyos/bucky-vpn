import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:vpn_web/prompt_dialog.dart';

import 'api.dart';
import 'input_dialog.dart';

class JoinedNodesPage extends StatefulWidget {
  const JoinedNodesPage({super.key});

  @override
  State<StatefulWidget> createState() => _JoinedNodesPageState();

}

class _JoinedNodesPageState extends State<JoinedNodesPage> {
  List<JoinedNode>? _joinedNodes;

  void refreshNodes() {
    Api.instance().getJoinedNodes().then((ret) {
      var (result, resp) = ret;
      if (result.isSuccess) {
        if (mounted) {
          setState(() {
            _joinedNodes = resp ?? [];
          });
        }
      } else {
        Fluttertoast.showToast(
            msg: result.msg ?? "获取节点列表失败",
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
    refreshNodes();
  }

  @override
  Widget build(BuildContext context) {
    if (_joinedNodes == null) {
      return const Scaffold(
        backgroundColor: Colors.white,
        body: Center(
          child: CircularProgressIndicator(),
        ),
      );
    } else {
      return Scaffold(
        backgroundColor: Colors.white,
        body: Container(
          padding: EdgeInsets.only(top: 20),
          color: Colors.white,
          child: Table(
              border: TableBorder.all(color: Colors.black),
              defaultVerticalAlignment: TableCellVerticalAlignment.middle,
              columnWidths: {
                0: FixedColumnWidth(120),
                1: FixedColumnWidth(120),
                2: FlexColumnWidth(),
                3: FixedColumnWidth(160),
                4: FixedColumnWidth(160)
              },
              children: [
                TableRow(
                  children: [
                    TableCell(
                        child: Center(
                          child: Text("Allow Join"),
                        )),
                    TableCell(child: Center(child: Text("Name"))),
                    TableCell(child: Center(child: Text("ID"))),
                    TableCell(child: Center(child: Text("State"))),
                    TableCell(child: Center(child: Text("Action")))
                  ],
                ),
                for (var node in _joinedNodes!)
                  TableRow(
                    children: [
                      TableCell(
                          child: Center(
                              child: Checkbox(
                                value: node.allowJoin,
                                onChanged: (value) async {
                                  if (value != null) {
                                    var result = await Api.instance().allowJoin(node.nodeId, value);
                                    if (result.isSuccess) {
                                      node.allowJoin = value;
                                    }
                                    setState(() {});
                                  }
                                },
                              ))),
                      TableCell(child: Center(child: Text(node.comment.isNotEmpty ? node.comment : node.name.isNotEmpty ? node.name : node.nodeId))),
                      TableCell(child: Center(child: Text(node.nodeId))),
                      TableCell(child: Center(child: Text(node.isOnline? node.ipList!.join("\n"):"offline"))),
                      TableCell(
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.spaceAround,
                            children: [
                              MouseRegion(
                                  cursor: SystemMouseCursors.click,
                                  child: InkWell(
                                      onTap: () async {
                                        showDialog(
                                            context: context,
                                            builder: (context) {
                                              return Dialog(
                                                backgroundColor: Colors.transparent,
                                                child: SizedBox(
                                                    width: 400,
                                                    height: 300,
                                                    child: InputDialog(
                                                      onOk: (comment) async {
                                                        var result =
                                                        await Api.instance()
                                                            .updateJoinComment(
                                                            node.nodeId, comment);
                                                        if (result.isSuccess) {
                                                          refreshNodes();
                                                        } else {
                                                          Fluttertoast.showToast(
                                                              msg: result.msg ??
                                                                  "update comment failed",
                                                              toastLength:
                                                              Toast.LENGTH_LONG,
                                                              gravity:
                                                              ToastGravity.TOP,
                                                              backgroundColor:
                                                              Colors.red,
                                                              textColor: Colors.white,
                                                              fontSize: 16.0,
                                                              timeInSecForIosWeb: 5);
                                                        }
                                                      }, defaultContent: node.comment, hintText: 'Please enter comment', title: 'Comment',
                                                    )),
                                              );
                                            });
                                      },
                                      child: Text(
                                        "comment",
                                        style: TextStyle(
                                            color: Colors.blue,
                                            decoration: TextDecoration.underline),
                                      ))),
                              MouseRegion(
                            cursor: SystemMouseCursors.click,
                            child: InkWell(
                                onTap: () async {
                                  showDialog(
                                      context: context,
                                      builder: (context) {
                                        return Dialog(
                                          backgroundColor: Colors.transparent,
                                          child: SizedBox(
                                              width: 400,
                                              height: 300,
                                              child: PromptDialog(
                                                promptTitle: "Delete Node",
                                                prompt:
                                                    "Are you sure to delete node ${node.name}?",
                                                onConfirm: () async {
                                                  var result =
                                                      await Api.instance()
                                                          .deleteJoinedNode(
                                                              node.nodeId);
                                                  if (result.isSuccess) {
                                                    refreshNodes();
                                                  } else {
                                                    Fluttertoast.showToast(
                                                        msg: result.msg ??
                                                            "delete failed",
                                                        toastLength:
                                                            Toast.LENGTH_LONG,
                                                        gravity:
                                                            ToastGravity.TOP,
                                                        backgroundColor:
                                                            Colors.red,
                                                        textColor: Colors.white,
                                                        fontSize: 16.0,
                                                        timeInSecForIosWeb: 5);
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
                                      decoration: TextDecoration.underline),
                                ))),
                      ])),
                    ],
                  )
              ]),
        ),
      );
    }
  }
}
