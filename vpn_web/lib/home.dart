import 'dart:async';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:vpn_web/networks_page.dart';

import 'api.dart';
import 'joined_nodes_page.dart';

class Home extends StatefulWidget {
  const Home({super.key});

  @override
  State<Home> createState() => _HomeState();
}

class _HomeState extends State<Home> with SingleTickerProviderStateMixin {
  UserInfo? _userInfo;
  Timer? _timer;
  late TabController _tabController;
  int _activeTabIndex = 0;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
    _tabController.addListener(() {
      if (!mounted) {
        return;
      }
      if (_activeTabIndex != _tabController.index) {
        setState(() {
          _activeTabIndex = _tabController.index;
        });
      }
    });

    Api.instance().getUserInfo().then((ret) {
      final (result, resp) = ret;
      if (!mounted) {
        return;
      }
      if (result.isSuccess) {
        setState(() {
          _userInfo = resp;
        });
      } else {
        context.go('/login');
      }
    });

    _timer = Timer.periodic(const Duration(seconds: 3000), (_) {
      Api.instance().refreshSession();
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    _tabController.dispose();
    super.dispose();
  }

  void _logout() {
    if (mounted) {
      context.go('/login');
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_userInfo == null) {
      return const Scaffold(
        body: Center(child: CircularProgressIndicator()),
      );
    }

    return Scaffold(
      body: Container(
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [Color(0xFFEFF6F8), Color(0xFFF7FAFC)],
          ),
        ),
        child: SafeArea(
          child: Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 1180),
              child: Padding(
                padding: const EdgeInsets.all(20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(
                      width: double.infinity,
                      padding: const EdgeInsets.fromLTRB(22, 20, 18, 20),
                      decoration: BoxDecoration(
                        color: Colors.white,
                        borderRadius: BorderRadius.circular(20),
                        boxShadow: const [
                          BoxShadow(
                            color: Color(0x100E2A3A),
                            blurRadius: 20,
                            offset: Offset(0, 8),
                          ),
                        ],
                      ),
                      child: Row(
                        children: [
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                const Text(
                                  'Server Dashboard',
                                  style: TextStyle(
                                    fontSize: 28,
                                    fontWeight: FontWeight.w700,
                                    color: Color(0xFF0E2A3A),
                                  ),
                                ),
                                const SizedBox(height: 10),
                                SelectableText(
                                  'Server ID: ${_userInfo!.serverId}',
                                  style: const TextStyle(
                                    fontSize: 14,
                                    color: Color(0xFF4B6675),
                                  ),
                                ),
                                const SizedBox(height: 4),
                                SelectableText(
                                  'Network Group ID: ${_userInfo!.networkId}',
                                  style: const TextStyle(
                                    fontSize: 14,
                                    color: Color(0xFF4B6675),
                                  ),
                                ),
                              ],
                            ),
                          ),
                          FilledButton.icon(
                            onPressed: _logout,
                            icon: const Icon(Icons.logout, size: 18),
                            label: const Text('Logout'),
                            style: FilledButton.styleFrom(
                              backgroundColor: const Color(0xFF14425A),
                              shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(10),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 18),
                    Expanded(
                      child: Container(
                        width: double.infinity,
                        padding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
                        decoration: BoxDecoration(
                          color: Colors.white,
                          borderRadius: BorderRadius.circular(20),
                          boxShadow: const [
                            BoxShadow(
                              color: Color(0x100E2A3A),
                              blurRadius: 20,
                              offset: Offset(0, 8),
                            ),
                          ],
                        ),
                        child: Column(
                          children: [
                            TabBar(
                              controller: _tabController,
                              onTap: (index) {
                                if (_activeTabIndex != index) {
                                  setState(() {
                                    _activeTabIndex = index;
                                  });
                                }
                              },
                              isScrollable: true,
                              tabAlignment: TabAlignment.start,
                              indicatorColor: const Color(0xFF0A7E8C),
                              indicatorWeight: 2.5,
                              indicatorSize: TabBarIndicatorSize.label,
                              dividerColor: const Color(0xFFD9E6EC),
                              labelColor: const Color(0xFF0E2A3A),
                              labelStyle: const TextStyle(
                                fontSize: 14,
                                fontWeight: FontWeight.w700,
                              ),
                              unselectedLabelColor: const Color(0xFF4B6675),
                              unselectedLabelStyle: const TextStyle(
                                fontSize: 14,
                                fontWeight: FontWeight.w500,
                              ),
                              tabs: const [
                                Tab(text: 'Joined Nodes'),
                                Tab(text: 'My Networks'),
                              ],
                            ),
                            const SizedBox(height: 14),
                            Expanded(
                              child: IndexedStack(
                                index: _activeTabIndex,
                                children: const [
                                  JoinedNodesPage(),
                                  NetworksPage()
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
