import 'dart:async';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:vpn_web/networks_page.dart';

import 'api.dart';
import 'joined_nodes_page.dart';

class Home extends StatefulWidget {
  const Home({Key? key}) : super(key: key);

  @override
  createState() => _HomeState();
}

class _HomeState extends State<Home> with SingleTickerProviderStateMixin {
  UserInfo? _userInfo;
  Timer? _timer;
  late TabController _tabController;

  @override
  void initState() {
    super.initState();
    Api.instance().getUserInfo().then((ret) {
      var (result, resp) = ret;
      if (result.isSuccess) {
        _userInfo = resp;
        setState(() {

        });
      } else {
        if (mounted) {
          context.go('/login');
        }
      }
    });
    _timer = Timer.periodic(Duration(seconds: 3000), (_timer) {
      Api.instance().refreshSession();
    });
    _tabController = TabController(length: 2, vsync: this);
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_userInfo == null) {
      return const Scaffold(
        body: Center(
          child: CircularProgressIndicator(),
        ),
      );
    } else {
      return Scaffold(
        backgroundColor: Colors.white,
        body: Center(
            child: Container(
              width: 600,
              color: Colors.white,
              child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              children: <Widget>[
                const SizedBox(height: 30),
                SelectableText(
                  "Server Id: ${_userInfo!.serverId}",
                  style: const TextStyle(fontSize: 30),
                ),
                const SizedBox(height: 30),
                SelectableText(
                  "Network Id: ${_userInfo!.networkId}",
                  style: const TextStyle(fontSize: 30),
                ),
                const SizedBox(height: 30),
                Expanded(
                    child: Column(
                      children: [
                        TabBar(controller: _tabController, tabs: [
                          Tab(text: "连接节点"),
                          Tab(text: "我的网络"),
                        ]),
                        Expanded(
                            child: TabBarView(controller: _tabController, children: [
                              JoinedNodesPage(),
                              NetworksPage(),
                            ]))
                      ],
                    ))
              ],
            ),
          ),
        ),
      );
    }
  }
}
