import 'package:flutter/material.dart';
import 'dialog_button.dart';


typedef ContentBuilder = Widget Function(BuildContext context);

abstract class DialogBase extends StatelessWidget {
  final String title;
  VoidCallback? onConfirm;
  VoidCallback? onCancel;
  final String? cancelText;
  final String? confirmText;
   bool pop;

  DialogBase(
      {super.key,
        this.onConfirm,
        this.onCancel,
        required this.title,
        this.cancelText,
        this.confirmText,
         this.pop = true});

  @protected
  Widget buildContent(BuildContext context);

  @override
  Widget build(BuildContext context) {
    return Dialog(
      insetPadding: const EdgeInsets.symmetric(horizontal: 14.0),
      child: SizedBox(
        child: Container(
          decoration: BoxDecoration(
            color: Colors.white,
            borderRadius: BorderRadius.circular(10.0),
          ),
          padding: const EdgeInsets.only(top: 17.0, bottom: 34),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                title,
                style:
                const TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
              ),
              buildContent(context),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                children: <Widget>[
                  DialogButton(
                    onPressed: () {
                      if (pop) {
                        Navigator.of(context).pop();
                      }
                      if (onCancel != null) {
                        onCancel!();
                      }
                    },
                    text: cancelText == null ? "Cancel" : cancelText!,
                  ),
                  DialogButton(
                    onPressed: () {
                      if (pop) {
                        Navigator.of(context).pop();
                      }
                      if (onConfirm != null) {
                        onConfirm!();
                      }
                    },
                    isDefault: true,
                    text: confirmText == null ? "Ok" : confirmText!,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
