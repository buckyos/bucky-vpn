import 'dart:async';

import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:go_router/go_router.dart';
import 'package:vpn_web/networks_page.dart';

import 'api.dart';
import 'joined_nodes_page.dart';
import 'proxy_nodes_page.dart';
import 'traffic_stats.dart';

class Home extends StatefulWidget {
  const Home({super.key});

  @override
  State<Home> createState() => _HomeState();
}

class _HomeState extends State<Home> with SingleTickerProviderStateMixin {
  UserInfo? _userInfo;
  TrafficStats? _userTrafficStats;
  Timer? _timer;
  late TabController _tabController;
  int _activeTabIndex = 0;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 3, vsync: this);
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

    _loadHomeData();

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

  Future<void> _loadHomeData() async {
    final userInfoFuture = Api.instance().getUserInfo();
    final trafficFuture = Api.instance().getUserTrafficStats();
    final (userResult, userInfo) = await userInfoFuture;
    final (trafficResult, trafficStats) = await trafficFuture;

    if (!mounted) {
      return;
    }

    if (!userResult.isSuccess || userInfo == null) {
      context.go('/login');
      return;
    }

    setState(() {
      _userInfo = userInfo;
      _userTrafficStats = trafficResult.isSuccess ? trafficStats : null;
    });
  }

  Future<void> _refreshTrafficStats() async {
    final (result, stats) = await Api.instance().getUserTrafficStats();
    if (!mounted) {
      return;
    }

    if (result.isSuccess) {
      setState(() {
        _userTrafficStats = stats;
      });
      return;
    }

    Fluttertoast.showToast(
      msg: result.msg ?? 'Read traffic statistics failed',
      toastLength: Toast.LENGTH_LONG,
      gravity: ToastGravity.TOP,
      backgroundColor: Colors.red,
      textColor: Colors.white,
      fontSize: 16.0,
      timeInSecForIosWeb: 5,
    );
  }

  Widget _buildTrafficMetricCard({
    required IconData icon,
    required String label,
    required String value,
    required Color iconColor,
    required Color backgroundColor,
  }) {
    return Container(
      width: 248,
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        color: backgroundColor,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: const Color(0xFFD9E6EC)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 22, color: iconColor),
          const SizedBox(height: 14),
          Text(
            label,
            style: const TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: Color(0xFF4B6675),
            ),
          ),
          const SizedBox(height: 8),
          Text(
            value,
            style: const TextStyle(
              fontSize: 24,
              fontWeight: FontWeight.w700,
              color: Color(0xFF0E2A3A),
            ),
          ),
        ],
      ),
    );
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
                    Container(
                      width: double.infinity,
                      padding: const EdgeInsets.fromLTRB(18, 18, 18, 18),
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
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              const Icon(
                                Icons.speed_outlined,
                                size: 18,
                                color: Color(0xFF4B6675),
                              ),
                              const SizedBox(width: 8),
                              const Text(
                                'Traffic Overview',
                                style: TextStyle(
                                  fontSize: 16,
                                  fontWeight: FontWeight.w700,
                                  color: Color(0xFF204153),
                                ),
                              ),
                              const Spacer(),
                              OutlinedButton.icon(
                                onPressed: _refreshTrafficStats,
                                icon: const Icon(Icons.refresh, size: 18),
                                label: const Text('Refresh'),
                              ),
                            ],
                          ),
                          const SizedBox(height: 6),
                          const Text(
                            'Aggregated statistics for the current network group.',
                            style: TextStyle(
                              fontSize: 13,
                              color: Color(0xFF4B6675),
                            ),
                          ),
                          const SizedBox(height: 14),
                          Wrap(
                            spacing: 12,
                            runSpacing: 12,
                            children: [
                              _buildTrafficMetricCard(
                                icon: Icons.upload_rounded,
                                label: 'Upload Speed',
                                value: _userTrafficStats == null
                                    ? '--'
                                    : formatTrafficSpeed(
                                        _userTrafficStats!.txSpeed),
                                iconColor: const Color(0xFF0A7E8C),
                                backgroundColor: const Color(0xFFF4FBFC),
                              ),
                              _buildTrafficMetricCard(
                                icon: Icons.download_rounded,
                                label: 'Download Speed',
                                value: _userTrafficStats == null
                                    ? '--'
                                    : formatTrafficSpeed(
                                        _userTrafficStats!.rxSpeed),
                                iconColor: const Color(0xFF2563EB),
                                backgroundColor: const Color(0xFFF4F8FF),
                              ),
                              _buildTrafficMetricCard(
                                icon: Icons.cloud_upload_outlined,
                                label: 'Upload Traffic',
                                value: _userTrafficStats == null
                                    ? '--'
                                    : formatTrafficBytes(
                                        _userTrafficStats!.txBytes),
                                iconColor: const Color(0xFF0E7490),
                                backgroundColor: const Color(0xFFF2FBFB),
                              ),
                              _buildTrafficMetricCard(
                                icon: Icons.cloud_download_outlined,
                                label: 'Download Traffic',
                                value: _userTrafficStats == null
                                    ? '--'
                                    : formatTrafficBytes(
                                        _userTrafficStats!.rxBytes),
                                iconColor: const Color(0xFF1D4ED8),
                                backgroundColor: const Color(0xFFF5F7FF),
                              ),
                            ],
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
                                Tab(text: 'Proxy Nodes'),
                              ],
                            ),
                            const SizedBox(height: 14),
                            Expanded(
                              child: IndexedStack(
                                index: _activeTabIndex,
                                children: const [
                                  JoinedNodesPage(),
                                  NetworksPage(),
                                  ProxyNodesPage(),
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
