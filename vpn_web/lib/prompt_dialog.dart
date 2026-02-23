import 'package:flutter/material.dart';

import 'dialog_button.dart';

class PromptDialog extends StatelessWidget {
  final String prompt;
  final String? promptTitle;
  final VoidCallback onConfirm;
  final VoidCallback? onCancel;

  const PromptDialog({
    super.key,
    required this.prompt,
    required this.onConfirm,
    this.onCancel,
    this.promptTitle,
  });

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
              promptTitle ?? 'Prompt',
              style: const TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.w700,
                color: Color(0xFF0E2A3A),
              ),
            ),
            const SizedBox(height: 12),
            Text(
              prompt,
              style: const TextStyle(fontSize: 14, color: Color(0xFF3B5563)),
            ),
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                DialogButton(
                  onPressed: () {
                    Navigator.of(context).pop();
                    onCancel?.call();
                  },
                  text: 'Cancel',
                  width: 96,
                ),
                const SizedBox(width: 10),
                DialogButton(
                  onPressed: () {
                    Navigator.of(context).pop();
                    onConfirm();
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
