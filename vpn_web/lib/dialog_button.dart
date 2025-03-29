import 'package:flutter/material.dart';

class DialogButton extends StatelessWidget {
  final VoidCallback onPressed;
  final bool isDefault;
  final String text;
  final double width;
  final double height;
  final ButtonStyle? style;

  const DialogButton({super.key, required this.text, required this.onPressed, this.isDefault = false, this.width = 120, this.height = 30, this.style});


  @override
  Widget build(BuildContext context) {
    if (!isDefault) {
      var style = ElevatedButton.styleFrom(
        fixedSize: Size(width, height),
        shadowColor: Colors.transparent,
        backgroundColor: Colors.white,
        side: const BorderSide(color: Colors.black),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10.0),
        ),
      );
      style = style.merge(this.style);
      return ElevatedButton(
        style: style,
        onPressed: () {
          onPressed();
        },
        child: Text(
          text,
          style: const TextStyle(color: Colors.black),
        ),
      );
    } else {
      var style = ElevatedButton.styleFrom(
            fixedSize: Size(width, height),
            shadowColor: Colors.transparent,
            side: const BorderSide(color: Colors.black),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10.0),
            ),
      );
      style = style.merge(this.style);

      return ElevatedButton(
        style: style,
        onPressed: () {
          onPressed();
        },
        child: Text(
          text,
          style: const TextStyle(color: Colors.black),
        ),
      );
    }
  }

}
