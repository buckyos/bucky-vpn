import 'package:flutter/material.dart';

import 'dialog_button.dart';

class InputDialog extends StatefulWidget {
  final String title;
  final String hintText;
  final Function(String) onOk;
  final String defaultContent;

  const InputDialog({
    super.key,
    required this.title,
    required this.hintText,
    required this.onOk,
    this.defaultContent = '',
  });

  @override
  State<InputDialog> createState() => _InputDialogState();
}

class _InputDialogState extends State<InputDialog> {
  late final TextEditingController _nameController;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.defaultContent);
  }

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      insetPadding: const EdgeInsets.symmetric(horizontal: 18),
      backgroundColor: Colors.transparent,
      child: Container(
        constraints: const BoxConstraints(maxWidth: 460),
        decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: BorderRadius.circular(18),
          boxShadow: const [
            BoxShadow(
              color: Color(0x240E2A3A),
              blurRadius: 24,
              offset: Offset(0, 10),
            ),
          ],
        ),
        padding: const EdgeInsets.fromLTRB(18, 18, 18, 16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              widget.title,
              style: const TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.w700,
                color: Color(0xFF0E2A3A),
              ),
            ),
            const SizedBox(height: 14),
            TextField(
              controller: _nameController,
              decoration: InputDecoration(
                hintText: widget.hintText,
                filled: true,
                fillColor: const Color(0xFFF7FBFD),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(10),
                  borderSide: BorderSide.none,
                ),
              ),
            ),
            const SizedBox(height: 14),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                DialogButton(
                  onPressed: () => Navigator.of(context).pop(),
                  text: 'Cancel',
                  width: 96,
                ),
                const SizedBox(width: 10),
                DialogButton(
                  onPressed: () {
                    if (_nameController.text.isEmpty) {
                      return;
                    }
                    Navigator.of(context).pop();
                    widget.onOk(_nameController.text);
                  },
                  isDefault: true,
                  text: 'Ok',
                  width: 96,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
