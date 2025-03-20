import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';

import 'api.dart';

class JoinedNodesPage extends StatefulWidget {
  const JoinedNodesPage({super.key});

  @override
  State<StatefulWidget> createState() => _JoinedNodesPageState();

}

class _JoinedNodesPageState extends State<JoinedNodesPage> {
  List<JoinedNode>? _joinedNodes;
  @override
  void initState() {
    super.initState();

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
                1: FixedColumnWidth(240),
                2: FlexColumnWidth(),
                3: FixedColumnWidth(160),
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
                    TableCell(child: Center(child: Text("State")))
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
                      TableCell(child: Center(child: Text(node.isOnline? node.ipList!.join("\n"):"offline")))
                    ],
                  )
              ]),
        ),
      );
    }
  }
}
