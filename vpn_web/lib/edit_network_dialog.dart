import 'package:flutter/material.dart';

import 'dialog_button.dart';

class EditNetworkDialog extends StatefulWidget {
  final String? name;
  final String? address;
  final int? mask;
  final Function(String, String, int) onSave;

  const EditNetworkDialog({
    super.key,
    this.name,
    this.address,
    this.mask,
    required this.onSave,
  });

  @override
  State<EditNetworkDialog> createState() => _EditNetworkDialogState();
}

class _EditNetworkDialogState extends State<EditNetworkDialog> {
  late TextEditingController _controllerName;
  late TextEditingController _controllerAddress;
  late TextEditingController _controllerMask;

  @override
  void initState() {
    super.initState();
    _controllerName = TextEditingController(text: widget.name ?? '');
    _controllerAddress = TextEditingController(text: widget.address ?? '');
    _controllerMask =
        TextEditingController(text: widget.mask?.toString() ?? '');
  }

  @override
  void dispose() {
    _controllerName.dispose();
    _controllerAddress.dispose();
    _controllerMask.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: Colors.transparent,
      child: Container(
        constraints: const BoxConstraints(maxWidth: 460),
        padding: const EdgeInsets.all(20),
        decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: BorderRadius.circular(18),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              widget.name == null ? 'Create Network' : 'Edit Network',
              style: const TextStyle(
                fontSize: 18,
                fontWeight: FontWeight.w700,
                color: Color(0xFF0E2A3A),
              ),
            ),
            const SizedBox(height: 16),
            TextField(
              controller: _controllerName,
              decoration: _inputDecoration('Network Name'),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _controllerAddress,
              decoration: _inputDecoration('Network Address'),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _controllerMask,
              keyboardType: TextInputType.number,
              decoration: _inputDecoration('Network Mask'),
            ),
            const SizedBox(height: 18),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                DialogButton(
                  onPressed: () => Navigator.of(context).pop(),
                  text: 'Cancel',
                  width: 98,
                ),
                const SizedBox(width: 10),
                DialogButton(
                  onPressed: () {
                    final mask = int.tryParse(_controllerMask.text);
                    if (_controllerName.text.isEmpty ||
                        _controllerAddress.text.isEmpty ||
                        mask == null) {
                      return;
                    }
                    Navigator.of(context).pop();
                    widget.onSave(
                      _controllerName.text,
                      _controllerAddress.text,
                      mask,
                    );
                  },
                  isDefault: true,
                  text: 'Save',
                  width: 98,
                ),
              ],
            )
          ],
        ),
      ),
    );
  }

  InputDecoration _inputDecoration(String label) {
    return InputDecoration(
      labelText: label,
      filled: true,
      fillColor: const Color(0xFFF7FBFD),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: BorderSide.none,
      ),
    );
  }
}
