import 'package:flutter/material.dart';

class DialogButton extends StatelessWidget {
  final VoidCallback onPressed;
  final bool isDefault;
  final String text;
  final double width;
  final double height;
  final ButtonStyle? style;

  const DialogButton({
    super.key,
    required this.text,
    required this.onPressed,
    this.isDefault = false,
    this.width = 120,
    this.height = 38,
    this.style,
  });

  @override
  Widget build(BuildContext context) {
    final baseStyle =
        (isDefault ? FilledButton.styleFrom : OutlinedButton.styleFrom)(
      fixedSize: Size(width, height),
      foregroundColor: isDefault ? Colors.white : const Color(0xFF0E2A3A),
      backgroundColor: isDefault ? const Color(0xFF0A7E8C) : Colors.white,
      side: isDefault
          ? BorderSide.none
          : const BorderSide(color: Color(0xFFB4C8D1)),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
      ),
      textStyle: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
    ).merge(style);

    if (isDefault) {
      return FilledButton(
        style: baseStyle,
        onPressed: onPressed,
        child: Text(text),
      );
    }

    return OutlinedButton(
      style: baseStyle,
      onPressed: onPressed,
      child: Text(text),
    );
  }
}
