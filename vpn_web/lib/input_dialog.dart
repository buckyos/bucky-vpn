
import 'package:flutter/material.dart';

import 'dialog_base.dart';

class InputDialog extends DialogBase {
  final String hintText;
  final Function(String) onOk;
  final String defaultContent;

  final TextEditingController nameController = TextEditingController();

  InputDialog({
    super.key,
    required this.hintText,
    required this.onOk,
    this.defaultContent = "",
    super.onCancel,
    super.cancelText,
    super.confirmText,
    required super.title,
  }) {
    super.pop = true;
    super.onConfirm = () {
      if (nameController.text.isEmpty) {
        return;
      }
      onOk(nameController.text);
    };
    super.onCancel = () {
    };
    nameController.text = defaultContent;
  }

  @override
  Widget buildContent(BuildContext context) {
    return Container(
      padding: const EdgeInsets.only(top: 30, bottom: 38, left: 15, right: 15),
      child: Container(
        padding: EdgeInsets.symmetric(horizontal: 19), // 设置输入框左右间距
        child: Column(
          children: [
            TextField(
              controller: nameController,
              textAlign: TextAlign.center, // 文字居中
              decoration: InputDecoration(
                hintText: hintText,
                hintStyle: const TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w400,
                ),
                border: InputBorder.none, // 移除默认下划线
                contentPadding: const EdgeInsets.symmetric(vertical: 8),
              ),
            ),
            Container(
              height: 1, // 设置 Divider 的高度为 1
              child: Divider(
                thickness: 1.5,
                color: Colors.grey[300],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
