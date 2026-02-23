import 'package:flutter/material.dart';

import 'dialog_button.dart';

abstract class DialogBase extends StatelessWidget {
  final String title;
  final VoidCallback? onConfirm;
  final VoidCallback? onCancel;
  final String? cancelText;
  final String? confirmText;
  final bool pop;

  const DialogBase({
    super.key,
    this.onConfirm,
    this.onCancel,
    required this.title,
    this.cancelText,
    this.confirmText,
    this.pop = true,
  });

  @protected
  Widget buildContent(BuildContext context);

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
              title,
              style: const TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.w700,
                color: Color(0xFF0E2A3A),
              ),
            ),
            const SizedBox(height: 10),
            buildContent(context),
            const SizedBox(height: 12),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                DialogButton(
                  onPressed: () {
                    if (pop) {
                      Navigator.of(context).pop();
                    }
                    onCancel?.call();
                  },
                  text: cancelText ?? 'Cancel',
                  width: 96,
                ),
                const SizedBox(width: 10),
                DialogButton(
                  onPressed: () {
                    if (pop) {
                      Navigator.of(context).pop();
                    }
                    onConfirm?.call();
                  },
                  isDefault: true,
                  text: confirmText ?? 'Ok',
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
