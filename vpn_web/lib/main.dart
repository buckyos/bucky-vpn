import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:vpn_web/home.dart';
import 'package:vpn_web/login.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  GoRouter buildRouter(BuildContext context) {
    return GoRouter(routes: [
      GoRoute(
        path: "/",
        builder: (context, state) {
          return const Home();
        },
      ),
      GoRoute(
        path: "/login",
        builder: (context, state) {
          return const Login();
        },
      ),
    ]);
  }

  // This widget is the root of your application.
  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      debugShowCheckedModeBanner: false,
      title: 'Bucky VPN',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF0A7E8C)),
        useMaterial3: true,
        fontFamily: 'Trebuchet MS',
        scaffoldBackgroundColor: const Color(0xFFF4F8FA),
        visualDensity: VisualDensity.compact,
        cardTheme: const CardThemeData(
          elevation: 0,
          color: Colors.white,
          margin: EdgeInsets.zero,
        ),
        filledButtonTheme: FilledButtonThemeData(
          style: FilledButton.styleFrom(
            backgroundColor: const Color(0xFF0A7E8C),
            foregroundColor: Colors.white,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
            ),
          ),
        ),
        inputDecorationTheme: InputDecorationTheme(
          filled: true,
          fillColor: const Color(0xFFF7FBFD),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(10),
            borderSide: const BorderSide(color: Color(0xFFD1E0E8)),
          ),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(10),
            borderSide: const BorderSide(color: Color(0xFFD1E0E8)),
          ),
        ),
        dataTableTheme: const DataTableThemeData(
          headingRowColor: WidgetStatePropertyAll(Color(0xFFEAF4F8)),
          headingTextStyle: TextStyle(
            fontWeight: FontWeight.w700,
            color: Color(0xFF204153),
          ),
          dataTextStyle: TextStyle(color: Color(0xFF1D3746)),
        ),
      ),
      routerConfig: buildRouter(context),
    );
  }
}
