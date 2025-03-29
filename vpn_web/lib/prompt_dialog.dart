import 'package:flutter/material.dart';

import 'dialog_base.dart';

class PromptDialog extends DialogBase {
  final String prompt;
  String? promptTitle;
  PromptDialog({
    super.key,
    required this.prompt,
    required super.onConfirm,
    super.onCancel,
    super.cancelText,
    super.confirmText,
    this.promptTitle
  }) : super(title: promptTitle ?? "Prompt");

  @override
  Widget buildContent(BuildContext context) {
    return Container(
        padding: const EdgeInsets.only(top: 34, bottom: 38, left: 15, right: 15),
        child: Text(
          prompt,
          style: const TextStyle(
              fontSize: 14, color: Colors.black),
        ));
  }

}
