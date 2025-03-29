import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:go_router/go_router.dart';
import 'package:vpn_web/dialog_button.dart';

import 'api.dart';

class Login extends StatefulWidget {
  const Login({super.key});

  @override
  createState() => _LoginState();
}

class _LoginState extends State<Login> {
  final TextEditingController _usernameController = TextEditingController();
  final TextEditingController _passwordController = TextEditingController();
  bool _isObscure = true;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.white,
      body: Center(
        child: Container(
          width: 400,
          height: 300,
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              TextField(
                controller: _usernameController,
                decoration: const InputDecoration(
                  labelText: 'Username',
                ),
              ),
              TextField(
                controller: _passwordController,
                decoration: InputDecoration(
                  labelText: 'Password',
                  suffixIcon: IconButton(
                    icon: Icon(_isObscure ? Icons.visibility : Icons.visibility_off),
                    onPressed: () {
                      setState(() {
                        _isObscure = !_isObscure;
                      });
                    },
                  ),
                ),
                obscureText: _isObscure,
              ),
              const SizedBox(height: 20),
              DialogButton(
                onPressed: () async {
                  if (_usernameController.text.isEmpty || _passwordController.text.isEmpty) {
                    Fluttertoast.showToast(
                      msg: "Username or password is empty",
                      toastLength: Toast.LENGTH_SHORT,
                      gravity: ToastGravity.TOP,
                      backgroundColor: Colors.black,
                      textColor: Colors.white,
                      fontSize: 16.0,
                    );
                    return;
                  }

                  final result = await Api.instance().login(_usernameController.text, _passwordController.text);
                  if (result.isSuccess) {
                    if (mounted) {
                      context.go('/');
                    }
                  } else {
                    Fluttertoast.showToast(
                      msg: result.msg ?? "Login failed",
                      toastLength: Toast.LENGTH_SHORT,
                      gravity: ToastGravity.TOP,
                      backgroundColor: Colors.black,
                      textColor: Colors.white,
                      fontSize: 16.0,
                    );
                  }
                },
                text: 'Login',
              ),
            ],
          ),
        ),
      ),
    );
  }
}
